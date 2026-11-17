use binder_core::Db;
use binder_core::events::EventKind;
use binder_core::uptime::{self, Watch};

const H: i64 = 3600;

#[test]
fn checks_add_up_to_buckets_and_a_percent() {
    let mut db = Db::in_memory().unwrap();
    let now = 100 * H;
    // Six hours a bucket over the last day: four buckets.
    for i in 0..24 {
        let ok = i != 20; // one failed check, in the last bucket
        uptime::record(&mut db, "blog", now - 24 * H + i * H, ok, Some(120)).unwrap();
    }
    uptime::record(&mut db, "shop", now - H, false, None).unwrap();
    let s = uptime::summary(&db, "blog", now, 24 * H, 4).unwrap();
    assert_eq!(s.buckets, vec![Some(true), Some(true), Some(true), Some(false)]);
    assert_eq!(s.percent.map(|p| (p * 10.0).round() / 10.0), Some(95.8));
    assert_eq!(s.ms, Some(120));
    let empty = uptime::summary(&db, "new", now, 24 * H, 4).unwrap();
    assert_eq!((empty.buckets, empty.percent, empty.ms), (vec![None, None, None, None], None, None));
}

#[test]
fn old_checks_are_pruned() {
    let mut db = Db::in_memory().unwrap();
    uptime::record(&mut db, "blog", 10, true, Some(1)).unwrap();
    uptime::record(&mut db, "blog", 100, true, Some(1)).unwrap();
    assert_eq!(uptime::prune(&mut db, 50).unwrap(), 1);
}

#[test]
fn down_needs_two_failures_in_a_row_and_back_comes_once() {
    let mut w = Watch::default();
    assert_eq!(w.saw("blog", false), None);
    assert_eq!(w.saw("blog", true), None); // one blip is not news
    assert_eq!(w.saw("blog", false), None);
    assert_eq!(w.saw("blog", false), Some(EventKind::SiteDown));
    assert_eq!(w.saw("blog", false), None);
    assert_eq!(w.saw("blog", true), Some(EventKind::SiteBack));
    assert_eq!(w.saw("blog", true), None);
}
