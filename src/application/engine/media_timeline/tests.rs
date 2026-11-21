use super::*;
mod basic {
    use super::*;
    use vjay_domain::{
        builtins::{ids, registry},
        node::AssetId,
    };
    #[test]
    fn pending_video_freezes_shared_timeline_and_range_edits_restart_it() {
        let factory = registry();
        let mut graph = Graph::default();
        let video = graph.add(ids::VIDEO, &factory).unwrap();
        graph.bind_asset(video, AssetId(0), &factory).unwrap();
        let mut signals = SignalEngine::default();
        signals.rebuild(&graph, &factory).unwrap();
        signals.tick(0.0);
        let mut timeline = MediaTimeline::default();
        timeline.rebuild(&graph, &signals).unwrap();
        timeline.tick_ready(0.0, &signals, |_| false);
        timeline.tick_ready(6.0, &signals, |_| false);
        assert_eq!(timeline.base(video), 0.0);
        assert!(!timeline.ready(video));
        timeline.tick_ready(6.1, &signals, |_| true);
        assert_eq!(timeline.base(video), 0.0);
        timeline.tick_ready(6.6, &signals, |_| true);
        assert!((timeline.base(video) - 0.5).abs() < 1e-6);
        graph.set_parameter(video, 3, 0.25, &factory).unwrap();
        signals.rebuild(&graph, &factory).unwrap();
        timeline.rebuild(&graph, &signals).unwrap();
        timeline.tick_ready(6.6, &signals, |_| true);
        assert_eq!(timeline.base(video), 0.0);
        assert_eq!(timeline.window(video).start, 0.25);
    }
}

mod seeking {
    use super::*;
    use vjay_domain::{
        builtins::{ids, registry},
        node::AssetId,
    };
    #[test]
    fn seek_waits_for_ready_media_and_end_does_not_wrap_to_start() {
        let factory = registry();
        let mut graph = Graph::default();
        let video = graph.add(ids::VIDEO, &factory).unwrap();
        graph.bind_asset(video, AssetId(0), &factory).unwrap();
        for (p, v) in [(1, 0.5), (3, 0.25), (4, 0.75)] {
            graph.set_parameter(video, p, v, &factory).unwrap();
        }
        let mut signals = SignalEngine::default();
        signals.rebuild(&graph, &factory).unwrap();
        signals.tick(0.0);
        let mut timeline = MediaTimeline::default();
        timeline.rebuild(&graph, &signals).unwrap();
        timeline.tick_ready(0.0, &signals, |_| false);
        timeline.seek(1.0);
        timeline.resolve_seeks(|_| Some(20.0));
        assert_eq!(timeline.base(video), 0.0);
        timeline.tick_ready(1.0, &signals, |_| true);
        timeline.resolve_seeks(|_| Some(20.0));
        let (time, _) = timeline
            .window(video)
            .resolve(timeline.base(video), 0.5, 20.0);
        assert!((time - 15.0).abs() < 0.00001);
        timeline.seek(0.0);
        timeline.resolve_seeks(|_| Some(20.0));
        assert_eq!(
            timeline
                .window(video)
                .resolve(timeline.base(video), 0.5, 20.0)
                .0,
            5.0
        );
        timeline.seek(0.5);
        timeline.resolve_seeks(|_| Some(20.0));
        assert_eq!(
            timeline
                .window(video)
                .resolve(timeline.base(video), 0.5, 20.0)
                .0,
            10.0,
            "ordinary seeks must not be shifted before an exact frame boundary"
        );
        for _ in 0..1000 {
            timeline
                .command(video, TransportAction::Jog(0.001))
                .unwrap();
            timeline.resolve_seeks(|_| Some(20.0));
        }
        assert!(
            (timeline.state(video).unwrap().seconds - 11.0).abs() < 1e-9,
            "repeated jogs must not accumulate normalized f32 rounding error"
        );
    }
}
