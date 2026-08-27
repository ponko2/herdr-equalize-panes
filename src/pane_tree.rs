pub mod plan;
pub mod ratio;

use self::{
    plan::{Adjustment, Side},
    ratio::Ratio,
};
use std::num::NonZeroU32;

#[derive(Debug, Clone, PartialEq)]
pub enum PaneTree {
    Pane,
    Split {
        ratio: Ratio,
        first: Box<PaneTree>,
        second: Box<PaneTree>,
    },
}

impl PaneTree {
    pub fn equalization_plan(&self) -> Vec<Adjustment> {
        let mut adjustments = Vec::new();
        self.collect_adjustments(&mut Vec::new(), &mut adjustments);
        adjustments
    }

    fn collect_adjustments(
        &self,
        path: &mut Vec<Side>,
        adjustments: &mut Vec<Adjustment>,
    ) -> NonZeroU32 {
        let Self::Split {
            ratio,
            first,
            second,
        } = self
        else {
            return NonZeroU32::MIN;
        };

        path.push(Side::First);
        let first_leaves = first.collect_adjustments(path, adjustments);
        path.pop();

        path.push(Side::Second);
        let second_leaves = second.collect_adjustments(path, adjustments);
        path.pop();

        let even = Ratio::from_leaf_counts(first_leaves, second_leaves);
        // NOTE: adjusting a parent first shifts the cells its children actually get
        if !ratio.approx_eq(even) {
            adjustments.push(Adjustment {
                path: path.clone(),
                ratio: even,
            });
        }
        first_leaves.saturating_add(second_leaves.get())
    }
}

#[cfg(test)]
mod tests {
    use super::{plan::Side, *};

    fn split(ratio: f64, first: PaneTree, second: PaneTree) -> PaneTree {
        PaneTree::Split {
            ratio: Ratio::try_from(ratio).expect("the fixture is a valid ratio"),
            first: Box::new(first),
            second: Box::new(second),
        }
    }

    fn every_shape(leaf_count: u32, ratio: f64) -> Vec<PaneTree> {
        if leaf_count == 1 {
            return vec![PaneTree::Pane];
        }
        (1..leaf_count)
            .flat_map(|first_leaves| {
                every_shape(first_leaves, ratio)
                    .into_iter()
                    .flat_map(move |first| {
                        every_shape(leaf_count - first_leaves, ratio)
                            .into_iter()
                            .map(move |second| split(ratio, first.clone(), second))
                    })
            })
            .collect()
    }

    fn equalized(tree: &PaneTree) -> PaneTree {
        let mut equalized = tree.clone();
        for adjustment in tree.equalization_plan() {
            let PaneTree::Split { ratio, .. } = walk(&mut equalized, &adjustment.path) else {
                panic!("a plan should never point a path at a pane");
            };
            *ratio = adjustment.ratio;
        }
        equalized
    }

    fn walk<'a>(tree: &'a mut PaneTree, path: &[Side]) -> &'a mut PaneTree {
        path.iter().fold(tree, |node, side| {
            let PaneTree::Split { first, second, .. } = node else {
                panic!("a plan should never point a path past a pane");
            };
            match side {
                Side::First => first,
                Side::Second => second,
            }
        })
    }

    fn pane_shares(tree: &PaneTree) -> Vec<f64> {
        fn collect(tree: &PaneTree, share: f64, shares: &mut Vec<f64>) {
            match tree {
                PaneTree::Pane => shares.push(share),
                PaneTree::Split {
                    ratio,
                    first,
                    second,
                } => {
                    collect(first, share * ratio.as_f64(), shares);
                    collect(second, share * (1.0 - ratio.as_f64()), shares);
                }
            }
        }
        let mut shares = Vec::new();
        collect(tree, 1.0, &mut shares);
        shares
    }

    fn chain_of_four(ratio: f64) -> PaneTree {
        split(
            ratio,
            split(
                ratio,
                split(ratio, PaneTree::Pane, PaneTree::Pane),
                PaneTree::Pane,
            ),
            PaneTree::Pane,
        )
    }

    fn is_ancestor_of(ancestor: &[Side], descendant: &[Side]) -> bool {
        descendant.starts_with(ancestor)
    }

    fn adjustment(path: &[Side], ratio: f64) -> Adjustment {
        Adjustment {
            path: path.to_vec(),
            ratio: Ratio::try_from(ratio).expect("the fixture is a valid ratio"),
        }
    }

    #[test]
    fn a_lone_pane_needs_no_adjustment() {
        assert_eq!(PaneTree::Pane.equalization_plan(), []);
    }

    #[test]
    fn a_split_that_already_shares_evenly_is_left_alone() {
        let tree = split(0.5, PaneTree::Pane, PaneTree::Pane);
        assert_eq!(tree.equalization_plan(), []);
    }

    #[test]
    fn a_lopsided_pair_is_halved() {
        let tree = split(0.2, PaneTree::Pane, PaneTree::Pane);
        assert_eq!(tree.equalization_plan(), [adjustment(&[], 0.5)]);
    }

    #[test]
    fn splitting_the_same_pane_twice_gives_the_lone_pane_a_third() {
        let tree = split(
            0.5,
            PaneTree::Pane,
            split(0.5, PaneTree::Pane, PaneTree::Pane),
        );
        assert_eq!(tree.equalization_plan(), [adjustment(&[], 1.0 / 3.0)]);
    }

    #[test]
    fn each_split_gets_the_share_of_the_panes_below_its_first_child() {
        assert_eq!(
            chain_of_four(0.5).equalization_plan(),
            [adjustment(&[Side::First], 2.0 / 3.0), adjustment(&[], 0.75),]
        );
    }

    #[test]
    fn descendants_are_adjusted_before_their_ancestors() {
        let plan = chain_of_four(0.1).equalization_plan();
        assert_eq!(plan.len(), 3, "every split should need one: {plan:?}");

        for (index, adjustment) in plan.iter().enumerate() {
            let after_an_ancestor = plan[..index]
                .iter()
                .any(|earlier| is_ancestor_of(&earlier.path, &adjustment.path));
            assert!(!after_an_ancestor, "{adjustment:?} follows its ancestor");
        }
    }

    #[test]
    fn a_split_you_resized_yourself_stays_put() {
        assert_eq!(
            split(0.5005, PaneTree::Pane, PaneTree::Pane).equalization_plan(),
            []
        );
        assert_eq!(
            split(0.502, PaneTree::Pane, PaneTree::Pane)
                .equalization_plan()
                .len(),
            1
        );
    }

    #[test]
    fn applying_a_plan_leaves_every_pane_with_an_equal_share() {
        for initial in [0.1, 0.5, 0.9] {
            for leaf_count in 1..=6 {
                for tree in every_shape(leaf_count, initial) {
                    let equalized = equalized(&tree);
                    let shares = pane_shares(&equalized);
                    let fair = 1.0 / f64::from(leaf_count);

                    assert_eq!(shares.len(), leaf_count as usize);
                    for share in shares {
                        assert!(
                            (share - fair).abs() < 1e-12,
                            "{share} is not {fair} in {equalized:?}"
                        );
                    }
                }
            }
        }
    }
}
