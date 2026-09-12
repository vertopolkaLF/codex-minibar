use super::*;
use crate::usage_overview::OverviewSnapshot;

#[derive(Clone, PartialEq)]
pub(super) struct Inputs {
    pub revision: u64,
    pub enabled: Vec<ProviderKind>,
    pub hour: DateTime<Local>,
}

#[derive(Clone, Copy, PartialEq)]
pub(super) enum Query {
    Spend(TotalSpendPeriod),
    Overview(OverviewMetric, OverviewRange),
}

pub(super) fn memoize(
    cx: &mut RenderCx,
    inputs: Inputs,
    query: Option<Query>,
    build: impl FnOnce() -> OverviewSnapshot,
) -> Arc<OverviewSnapshot> {
    cx.use_memo((inputs, query), || {
        Arc::new(if query.is_some() {
            build()
        } else {
            OverviewSnapshot::default()
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{cell::Cell, rc::Rc};

    #[test]
    fn hover_reuses_snapshot_but_data_period_and_clock_changes_recompute() {
        let mut cx = RenderCx::new(Rc::new(|| {}));
        let builds = Cell::new(0);
        let mut inputs = Inputs {
            revision: 0,
            enabled: vec![ProviderKind::Codex],
            hour: crate::usage::truncate_local_hour(Local::now()),
        };
        let render = |cx: &mut RenderCx, inputs: Inputs, query: Option<Query>| {
            cx.begin_render();
            memoize(cx, inputs, query, || {
                builds.set(builds.get() + 1);
                OverviewSnapshot::default()
            })
        };
        let spend = Some(Query::Spend(TotalSpendPeriod::ThirtyDays));
        let first = render(&mut cx, inputs.clone(), spend);
        for _ in 0..60 {
            assert!(Arc::ptr_eq(&first, &render(&mut cx, inputs.clone(), spend)));
        }
        assert_eq!(
            builds.get(),
            1,
            "moving across bars must not query the store again"
        );
        inputs.revision += 1;
        render(&mut cx, inputs.clone(), spend);
        assert_eq!(builds.get(), 2);
        inputs.enabled.push(ProviderKind::Claude);
        render(&mut cx, inputs.clone(), spend);
        assert_eq!(builds.get(), 3);
        inputs.hour += ChronoDuration::hours(1);
        render(&mut cx, inputs.clone(), spend);
        assert_eq!(builds.get(), 4);
        render(
            &mut cx,
            inputs.clone(),
            Some(Query::Spend(TotalSpendPeriod::Today)),
        );
        assert_eq!(builds.get(), 5);
        render(
            &mut cx,
            inputs.clone(),
            Some(Query::Overview(
                OverviewMetric::Tokens,
                OverviewRange::NinetyDays,
            )),
        );
        assert_eq!(builds.get(), 6);
        render(&mut cx, inputs, None);
        assert_eq!(builds.get(), 6, "hidden summaries must not query the store");
    }
}
