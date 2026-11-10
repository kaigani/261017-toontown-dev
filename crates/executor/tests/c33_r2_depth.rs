//! Seeded depth oracle against public descriptions.
mod c33_support;
mod common;
use common::{fixture, ok};
use serde_json::json;
#[test]
fn seeded_anchor_groups_4096() -> Result<(), Box<dyn std::error::Error>> {
    let h = fixture();
    // Remove fixture content: omitted entries also participate in render-list grouping.
    // This corpus owns every member of its role tier, so its oracle has no hidden anchors.
    ok(
        &h,
        "element.delete",
        json!({"elementIds":[c33_support::BOB,c33_support::BACKGROUND,"scene:\"Intro\"/element:\"Notes\""]}),
    );
    let colour = c33_support::solids(&h, 1)[0].0.clone();
    for i in 0..6 {
        c33_support::square(&h, &format!("Oracle {i}"), 50.0, &colour);
    }
    let base = ok(&h, "inspect.frame.describe", json!({"detail":"full"}));
    let visible: Vec<String> = base["elements"]
        .as_array()
        .ok_or("elements")?
        .iter()
        .filter_map(|e| e["elementId"].as_str().map(str::to_owned))
        .collect();
    let doc = h.snapshot()?.ok_or("document")?;
    let scene = doc.scenes().find(|s| s.name() == "Intro").ok_or("scene")?;
    let mut ids: Vec<String> =
        scene.tree().iter().map(|n| n.id.as_str().to_owned()).filter(|id| visible.contains(id)).collect();
    ids.reverse();
    assert_eq!(ids.len(), 6);
    let mut timeline: Vec<String> = scene.tree().iter().map(|n| n.id.as_str().to_owned()).collect();
    let mut state = 0x5eedu64;
    for case in 0..4096 {
        for end in (1..timeline.len()).rev() {
            state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
            let pick = ((state >> 32) as usize) % (end + 1);
            timeline.swap(end, pick);
        }
        ok(&h, "element.order.set", json!({"order":"timeline","elementIds":timeline,"sceneId":c33_support::INTRO}));
        ids = timeline.iter().filter(|id| visible.contains(id)).rev().cloned().collect();
        let mut depths = Vec::new();
        for id in &ids {
            state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
            let z = ((state >> 32) % 7001) as f64 * 1e-9;
            depths.push(z);
            ok(&h, "anim.transform.set", json!({"nodeIds":[id],"target":"static","position":{"z":z}}));
        }
        let mut sorted: Vec<usize> = (0..ids.len()).collect();
        sorted.sort_by(|a, b| depths[*a].total_cmp(&depths[*b]));
        let mut want = Vec::new();
        let mut i = 0;
        while i < sorted.len() {
            let anchor = depths[sorted[i]];
            let mut end = i + 1;
            while end < sorted.len() && depths[sorted[end]] - anchor < 1e-6 {
                end += 1;
            }
            let mut group = sorted[i..end].to_vec();
            group.sort_unstable();
            want.extend(group.iter().map(|j| ids[*j].clone()));
            i = end;
        }
        let got = ok(&h, "inspect.frame.describe", json!({"detail":"full"}));
        let actual: Vec<String> = got["elements"]
            .as_array()
            .ok_or("elements")?
            .iter()
            .filter_map(|e| e["elementId"].as_str().map(str::to_owned))
            .collect();
        assert_eq!(actual, want, "seed case {case}; ids={ids:?}; depths={depths:?}; description={got}");
        if case % 64 == 0 {
            let hits = ok(&h, "inspect.hitTest", json!({"space":"image","point":{"x":960,"y":540}}));
            let hit_ids: Vec<String> = hits["hits"]
                .as_array()
                .ok_or("hits")?
                .iter()
                .filter_map(|e| e["elementId"].as_str().map(str::to_owned))
                .collect();
            let reversed: Vec<String> = want.iter().rev().cloned().collect();
            assert_eq!(hit_ids, reversed, "picking reverses composite order");
            let away = depths.iter().all(|a| depths.iter().all(|b| ((a - b).abs() - 1e-6).abs() > 1e-12));
            if away {
                for (id, z) in ids.iter().zip(&depths) {
                    ok(&h, "anim.transform.set", json!({"nodeIds":[id],"target":"static","position":{"z":z+1e-13}}));
                }
                let perturbed = ok(&h, "inspect.frame.describe", json!({"detail":"full"}));
                let order: Vec<String> = perturbed["elements"]
                    .as_array()
                    .ok_or("elements")?
                    .iter()
                    .filter_map(|e| e["elementId"].as_str().map(str::to_owned))
                    .collect();
                assert_eq!(order, want, "sub-1e-12 perturbation away from boundaries");
            }
        }
    }
    Ok(())
}
