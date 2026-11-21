use super::*;
use std::collections::{HashMap, HashSet};
use vjay_domain::node::AssetId;
/// Namespaces are resolved while loading/editing, never during playback.
pub fn normalize(
    doc: &mut ProjectDocument,
    existing: &HashSet<NodeId>,
    existing_assets: &HashSet<AssetId>,
) -> PortResult<()> {
    let mut nodes = existing.clone();
    let mut assets = existing_assets.clone();
    let mut next_node = nodes
        .iter()
        .map(|n| n.0)
        .chain(maxima(doc).0)
        .max()
        .unwrap_or(0)
        .checked_add(1)
        .ok_or("Node IDs exhausted")?;
    let mut next_asset = assets
        .iter()
        .map(|a| a.0)
        .chain(maxima(doc).1)
        .max()
        .unwrap_or(0)
        .checked_add(1)
        .ok_or("Asset IDs exhausted")?;
    visit(
        doc,
        &mut nodes,
        &mut assets,
        &mut next_node,
        &mut next_asset,
        0,
    )
}
fn maxima(doc: &ProjectDocument) -> (Option<u64>, Option<u64>) {
    let mut n = doc.nodes.iter().map(|n| n.id.0).max();
    let mut a = doc.assets.iter().map(|a| a.id.0).max();
    for c in &doc.subpatches {
        let (cn, ca) = maxima(&c.document);
        n = n.max(cn);
        a = a.max(ca);
    }
    (n, a)
}
fn visit(
    doc: &mut ProjectDocument,
    nodes: &mut HashSet<NodeId>,
    assets: &mut HashSet<AssetId>,
    next: &mut u64,
    next_asset: &mut u64,
    depth: usize,
) -> PortResult<()> {
    if depth > 8 || nodes.len() + doc.nodes.len() > 4096 {
        return Err("Subpatch hierarchy exceeds limits".into());
    }
    let mut mapping = HashMap::new();
    let mut local = HashSet::new();
    for n in &mut doc.nodes {
        let old = n.id;
        if !local.insert(old) {
            return Err("Duplicate node ID".into());
        }
        if !nodes.insert(old) {
            n.id = NodeId(*next);
            *next = next.checked_add(1).ok_or("Node IDs exhausted")?;
            nodes.insert(n.id);
        }
        mapping.insert(old, n.id);
    }
    let mut asset_map = HashMap::new();
    let mut local_assets = HashSet::new();
    for a in &mut doc.assets {
        let old = a.id;
        if !local_assets.insert(old) {
            return Err("Duplicate asset ID".into());
        }
        if !assets.insert(old) {
            a.id = AssetId(*next_asset);
            *next_asset = next_asset.checked_add(1).ok_or("Asset IDs exhausted")?;
            assets.insert(a.id);
        }
        asset_map.insert(old, a.id);
    }
    for n in &mut doc.nodes {
        if let Some(bookmark) = &mut n.settings.transport {
            bookmark.asset = bookmark.asset.and_then(|a| asset_map.get(&a).copied());
        }
        if let Some(a) = n.asset {
            n.asset = Some(*asset_map.get(&a).ok_or("Missing media asset")?);
        }
    }
    let map = |id: NodeId| {
        mapping
            .get(&id)
            .copied()
            .ok_or_else(|| "Invalid subpatch node reference".to_string())
    };
    for e in &mut doc.connections {
        e.from = map(e.from)?;
        e.to = map(e.to)?;
    }
    for p in &mut doc.positions {
        p.node = map(p.node)?;
    }
    for c in &mut doc.controls {
        c.node = map(c.node)?;
    }
    doc.output = doc.output.map(map).transpose()?;
    for c in &mut doc.subpatches {
        c.node = map(c.node)?;
        visit(&mut c.document, nodes, assets, next, next_asset, depth + 1)?;
    }
    Ok(())
}
