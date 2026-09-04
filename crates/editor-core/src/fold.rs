use crate::{EditorError, Result};

/// A foldable inclusive line range. The first line stays visible when collapsed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FoldRegion {
    pub start_line: u32,
    pub end_line: u32,
    pub collapsed: bool,
}

impl FoldRegion {
    pub fn new(start_line: u32, end_line: u32) -> Result<Self> {
        if end_line <= start_line {
            return Err(EditorError::InvalidFoldRegion {
                start_line,
                end_line,
            });
        }
        Ok(Self {
            start_line,
            end_line,
            collapsed: false,
        })
    }
}

/// Ordered fold regions supporting nested folds.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FoldSet {
    regions: Vec<FoldRegion>,
}

impl FoldSet {
    #[must_use]
    pub fn regions(&self) -> &[FoldRegion] {
        &self.regions
    }

    pub fn set_regions(&mut self, mut regions: Vec<FoldRegion>) {
        regions.sort_by_key(|region| (region.start_line, std::cmp::Reverse(region.end_line)));
        regions.dedup_by_key(|region| (region.start_line, region.end_line));
        self.regions = regions;
    }

    pub fn toggle_at(&mut self, start_line: u32) -> bool {
        if let Some(region) = self
            .regions
            .iter_mut()
            .find(|region| region.start_line == start_line)
        {
            region.collapsed = !region.collapsed;
            region.collapsed
        } else {
            false
        }
    }

    #[must_use]
    pub fn is_line_hidden(&self, line: u32) -> bool {
        self.regions
            .iter()
            .any(|region| region.collapsed && line > region.start_line && line <= region.end_line)
    }

    pub fn unfold_containing(&mut self, line: u32) {
        for region in &mut self.regions {
            if region.collapsed && line > region.start_line && line <= region.end_line {
                region.collapsed = false;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fold_regions_sort_toggle_and_hide_lines() {
        let mut folds = FoldSet::default();
        folds.set_regions(vec![
            FoldRegion::new(10, 14).expect("outer fold"),
            FoldRegion::new(2, 6).expect("inner fold"),
        ]);
        assert_eq!(
            folds.regions(),
            &[
                FoldRegion {
                    start_line: 2,
                    end_line: 6,
                    collapsed: false
                },
                FoldRegion {
                    start_line: 10,
                    end_line: 14,
                    collapsed: false
                },
            ]
        );
        assert!(folds.toggle_at(2));
        assert!(folds.is_line_hidden(3));
        assert!(!folds.is_line_hidden(2));
        folds.unfold_containing(4);
        assert!(!folds.is_line_hidden(4));
    }

    #[test]
    fn invalid_regions_are_rejected() {
        assert!(matches!(
            FoldRegion::new(7, 7),
            Err(EditorError::InvalidFoldRegion {
                start_line: 7,
                end_line: 7
            })
        ));
    }
}
