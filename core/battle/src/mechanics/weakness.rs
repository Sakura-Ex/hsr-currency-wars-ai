//! Natural and modifier-contributed weakness sets; no implicit resistance change.
use crate::elements::Element;
use std::collections::BTreeSet;

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "diagnostics", derive(serde::Serialize))]
/// 自然弱点、活动状态附加弱点及其并集。
pub struct WeaknessState {
    /// 输入提供的自然弱点集合。
    pub natural: BTreeSet<Element>,
    /// 当前活动状态贡献的附加弱点。
    pub attached: BTreeSet<Element>,
    /// 自然与附加弱点的去重并集。
    pub all: BTreeSet<Element>,
}

impl WeaknessState {
    /// 分别去重自然与附加弱点，并计算二者并集。
    pub fn compose(natural: &[Element], attached: impl IntoIterator<Item = Element>) -> Self {
        let natural: BTreeSet<_> = natural.iter().copied().collect();
        let attached: BTreeSet<_> = attached.into_iter().collect();
        let all = natural.union(&attached).copied().collect();
        Self {
            natural,
            attached,
            all,
        }
    }
}
