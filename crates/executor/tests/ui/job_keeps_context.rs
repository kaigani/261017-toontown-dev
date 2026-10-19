// Background work (jobs) runs on its own thread with `'static` closures: it cannot keep the mutation context.
use std::sync::Arc;

fn handler(cx: &mut tt_executor::Cx, jobs: &Arc<tt_executor::JobTable>) {
    let _ = jobs.submit("job.test", "user", move |_| {
        cx.patch_objects(Vec::new()).map(|()| serde_json::Value::Null)
    });
}

fn main() {
    let _ = handler;
}
