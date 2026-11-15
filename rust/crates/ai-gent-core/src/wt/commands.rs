//! The seven subcommands. They write to an [`Out`]; the binary prints it.
use super::engine::{
    External, Volume, ctr, dangling, engine, external, ids, labelled_volumes, release,
};
use super::git::{chain_dev, exclude, git, here, rel, root, worktrees};
use super::ports::{load, next_offset, save, write};
use super::procs::builds_running;
use super::{
    DIR, Failed, GB, LABEL_PROJECT, LABEL_WORKTREE, Out, Res, disk, pad, project, readme, settings,
    slug,
};
use crate::tokens::pyjson::{J, dumps, float_repr, obj, parse};
use std::path::{Path, PathBuf};

/// `plan [--json]`.
pub fn plan(json: bool, out: &mut Out) -> Res<()> {
    readme::plan(&here()?, json, out)
}

fn live_branches(r: &str) -> Res<Vec<Option<String>>> {
    let mut live: Vec<Option<String>> = Vec::new();
    for w in worktrees(r)? {
        if !live.contains(&w.branch) {
            live.push(w.branch);
        }
    }
    Ok(live)
}

/// `add BRANCH [--base B] [--min-free-gb N]`.
pub fn add(branch: &str, base: Option<&str>, min_free_gb: Option<f64>, out: &mut Out) -> Res<()> {
    let r = root()?;
    let base = match base {
        Some(b) => b.to_string(),
        None => chain_dev(&r)?,
    };
    let sl = slug(branch);
    let path = format!("{r}/{DIR}/{sl}");
    if Path::new(&path).exists() {
        return Err(Failed(format!("{DIR}/{sl} exists")));
    }
    if !git(
        &[
            "rev-parse",
            "--verify",
            "--quiet",
            &format!("refs/heads/{branch}"),
        ],
        Some(&r),
        true,
    )?
    .is_empty()
    {
        return Err(Failed(format!(
            "branch {branch} exists; a track starts on a new branch"
        )));
    }
    let live = live_branches(&r)?;
    if let Some(e) = engine() {
        disk::preflight(&r, &e, live.len(), min_free_gb, out)?;
    } else {
        out.eline("no container engine found: skipping the disk check");
    }
    exclude(&r)?;
    let mut ports: Vec<(String, i64)> = load(&r)
        .into_iter()
        .filter(|(b, _)| live.contains(&Some(b.clone())))
        .collect();
    let offset = next_offset(&ports);
    git(
        &["worktree", "add", "-q", "-b", branch, &path, &base],
        Some(&r),
        false,
    )?;
    ports.retain(|(b, _)| b != branch);
    ports.push((branch.to_string(), offset));
    save(&r, &ports)?;
    let p = project(&r);
    let compose = format!("{p}-{sl}");
    let j = obj(vec![
        ("branch", J::s(branch)),
        ("base", J::s(base)),
        ("path", J::s(path)),
        ("compose_project", J::s(compose.clone())),
        ("port_offset", J::Int(offset)),
        (
            "labels",
            obj(vec![
                (LABEL_PROJECT, J::s(p.clone())),
                (LABEL_WORKTREE, J::s(format!("{p}/{sl}"))),
            ]),
        ),
    ]);
    out.line(dumps(&j, true, Some(2)));
    out.eline(format!(
        "\nin this worktree: COMPOSE_PROJECT_NAME={compose} AI_GENT_PROJECT={p} AI_GENT_WORKTREE={sl}; publish ports +{offset} through an untracked compose.worktree.yaml (already excluded), never by editing the shared compose file; create volumes and containers with both labels (containers.md \u{a7}9a)"
    ));
    Ok(())
}

/// `list [--json]`.
pub fn list(json: bool, out: &mut Out) -> Res<()> {
    let r = root()?;
    let base = chain_dev(&r).unwrap_or_default();
    let mut rows = Vec::new();
    for w in worktrees(&r)? {
        let dirty = !git(&["status", "--porcelain"], Some(&w.path), true)?.is_empty();
        let (mut ahead, mut behind) = (None, None);
        if let (false, Some(b)) = (base.is_empty(), &w.branch) {
            let counts = git(
                &[
                    "rev-list",
                    "--left-right",
                    "--count",
                    &format!("{base}...{b}"),
                ],
                Some(&r),
                true,
            )?;
            let c: Vec<&str> = counts.split_whitespace().collect();
            if c.len() == 2 {
                behind = c[0].parse::<i64>().ok();
                ahead = c[1].parse::<i64>().ok();
            }
        }
        rows.push((
            w.branch.unwrap_or_else(|| "(detached)".into()),
            rel(&w.path, &r).to_string(),
            dirty,
            ahead,
            behind,
        ));
    }
    if json {
        let j = J::Arr(
            rows.iter()
                .map(|(b, p, d, a, bh)| {
                    obj(vec![
                        ("branch", J::s(b.clone())),
                        ("path", J::s(p.clone())),
                        ("dirty", J::Bool(*d)),
                        ("ahead", J::opt_i(*a)),
                        ("behind", J::opt_i(*bh)),
                    ])
                })
                .collect(),
        );
        out.line(dumps(&j, true, Some(2)));
        return Ok(());
    }
    let n = |x: &Option<i64>| x.map_or("None".to_string(), |v| v.to_string());
    for (b, p, d, a, bh) in &rows {
        out.line(format!(
            "{} {} +{}/-{} vs {}{}",
            pad(b, 36),
            pad(p, 44),
            n(a),
            n(bh),
            if base.is_empty() { "?" } else { &base },
            if *d { "  uncommitted changes" } else { "" }
        ));
    }
    if rows.is_empty() {
        out.line(format!("no worktrees under {DIR}/"));
    }
    Ok(())
}

/// `remove BRANCH [--keep-containers]`.
pub fn remove(branch: &str, keep_containers: bool, out: &mut Out) -> Res<()> {
    let r = root()?;
    let Some(w) = worktrees(&r)?
        .into_iter()
        .find(|w| w.branch.as_deref() == Some(branch))
    else {
        return Err(Failed(format!("no worktree under {DIR}/ for {branch}")));
    };
    let p = w.path;
    let rp = rel(&p, &r).to_string();
    if !git(&["status", "--porcelain"], Some(&p), true)?.is_empty() {
        return Err(Failed(format!(
            "{rp} has uncommitted changes; commit or discard them first"
        )));
    }
    let base = chain_dev(&r)?;
    // Landed means every change is on dev: by hash, or by content after the platform rebased it
    // (git cherry marks those "-").
    let unlanded = git(&["cherry", &base, branch], Some(&r), false)?
        .lines()
        .filter(|l| l.starts_with('+'))
        .count();
    if unlanded > 0 {
        return Err(Failed(format!(
            "{branch} has {unlanded} change(s) {base} doesn't; land it first"
        )));
    }
    let e = engine();
    let mut removed = Vec::new();
    if let (Some(e), false) = (&e, keep_containers) {
        removed = release(e, &r, &slug(branch))?;
    }
    git(&["worktree", "remove", &p], Some(&r), false)?;
    let deleted = git(&["branch", "-d", branch], Some(&r), true)?;
    if deleted.is_empty()
        && !git(
            &[
                "rev-parse",
                "--verify",
                "--quiet",
                &format!("refs/heads/{branch}"),
            ],
            Some(&r),
            true,
        )?
        .is_empty()
    {
        git(&["branch", "-D", branch], Some(&r), false)?; // its changes are on dev under new hashes (checked above)
    }
    git(&["worktree", "prune"], Some(&r), false)?;
    let mut ports = load(&r);
    if ports.iter().any(|(b, _)| b == branch) {
        ports.retain(|(b, _)| b != branch);
        save(&r, &ports)?;
    }
    out.line(format!("removed {rp} and branch {branch}"));
    for x in removed {
        out.line(format!("removed {x}"));
    }
    if e.is_none() {
        out.eline("no container engine found: the track's containers and volumes weren't checked");
    }
    Ok(())
}

fn need_engine() -> Res<String> {
    engine().ok_or_else(|| Failed("no container engine found (set CONTAINER_ENGINE)".into()))
}

fn baseline_file(r: &str) -> PathBuf {
    Path::new(r).join(DIR).join(".containers-baseline.json")
}

/// `baseline`.
pub fn baseline(out: &mut Out) -> Res<()> {
    let r = root()?;
    let e = need_engine()?;
    let rows = external(&e)?;
    let now = i64::try_from(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_secs()),
    )
    .unwrap_or(0);
    let j = obj(vec![
        (
            "taken",
            J::s(format!("{}Z", crate::tokens::util::fmt_epoch(now))),
        ),
        (
            "external",
            J::Arr(rows.iter().map(|x| J::s(x.id.clone())).collect()),
        ),
    ]);
    write(&baseline_file(&r), &(dumps(&j, true, Some(2)) + "\n"))?;
    out.line(format!(
        "baseline: {} external container(s) already in the store; `wt.py finish` leaves them alone",
        rows.len()
    ));
    Ok(())
}

fn load_baseline(r: &str) -> Option<Vec<String>> {
    let text = std::fs::read(baseline_file(r)).ok()?;
    match parse(&String::from_utf8_lossy(&text))?.get("external")? {
        J::Arr(a) => Some(
            a.iter()
                .filter_map(|x| x.as_str().map(str::to_string))
                .collect(),
        ),
        _ => None,
    }
}

fn round1(x: f64) -> f64 {
    format!("{x:.1}").parse().unwrap_or(x)
}

/// `resources [--json]`.
pub fn resources(json: bool, out: &mut Out) -> Res<()> {
    let r = root()?;
    let e = need_engine()?;
    let (total, free, path) = disk::store_usage(&e)?;
    let s = settings(&r);
    let floor = s
        .get("disk_floor_gb")
        .map_or_else(|| (20.0 * GB).max((total / 10) as f64), |f| f * GB);
    let mut live: Vec<String> = worktrees(&r)?
        .iter()
        .map(|w| slug(w.branch.as_deref().unwrap_or("")))
        .collect();
    live.push("main".into());
    let base = load_baseline(&r);
    let p = project(&r);
    let label = format!("label={LABEL_PROJECT}={p}");
    let images = ids(&e, &["images", "-q", "--filter", &label])?;
    let dangling_images = dangling(&e, &r)?;
    let volumes: Vec<(Volume, bool)> = labelled_volumes(&e, &r)?
        .into_iter()
        .map(|v| {
            let orphan = !live.contains(&v.track);
            (v, orphan)
        })
        .collect();
    let containers = ids(&e, &["ps", "-aq", "--filter", &label])?;
    let external_new: Vec<External> = external(&e)?
        .into_iter()
        .filter(|x| base.as_ref().is_none_or(|b| !b.contains(&x.id)))
        .collect();
    let (free_gb, floor_gb) = (round1(free as f64 / GB), round1(floor / GB));
    let below = (free as f64) < floor;
    if json {
        let j = obj(vec![
            ("project", J::s(p)),
            (
                "disk",
                obj(vec![
                    ("path", J::s(path)),
                    ("free_gb", J::Float(free_gb)),
                    ("floor_gb", J::Float(floor_gb)),
                    ("below_floor", J::Bool(below)),
                ]),
            ),
            ("images", J::strs(&images)),
            ("dangling_images", J::strs(&dangling_images)),
            (
                "volumes",
                J::Arr(
                    volumes
                        .iter()
                        .map(|(v, o)| {
                            obj(vec![
                                ("name", J::s(v.name.clone())),
                                ("track", J::s(v.track.clone())),
                                ("orphan", J::Bool(*o)),
                            ])
                        })
                        .collect(),
                ),
            ),
            ("containers", J::strs(&containers)),
            (
                "external_new",
                J::Arr(
                    external_new
                        .iter()
                        .map(|x| {
                            obj(vec![
                                ("id", J::s(x.id.clone())),
                                ("name", J::s(x.name.clone())),
                            ])
                        })
                        .collect(),
                ),
            ),
            ("baseline", J::Bool(base.is_some())),
        ]);
        out.line(dumps(&j, true, Some(2)));
        return Ok(());
    }
    out.line(format!(
        "disk: {} GB free on {path} (floor {} GB){}",
        float_repr(free_gb),
        float_repr(floor_gb),
        if below { "  BELOW THE FLOOR" } else { "" }
    ));
    out.line(format!(
        "images labelled {p}: {}, dangling: {}",
        images.len(),
        dangling_images.len()
    ));
    for (v, o) in &volumes {
        out.line(format!(
            "volume {} track {}{}",
            pad(&v.name, 48),
            v.track,
            if *o { "  (worktree gone)" } else { "" }
        ));
    }
    out.line(format!("containers labelled {p}: {}", containers.len()));
    for x in &external_new {
        out.line(format!(
            "external {} {}{}",
            x.id,
            x.name,
            if base.is_some() {
                ""
            } else {
                "  (no baseline: not attributable)"
            }
        ));
    }
    out.line("report only; `wt.py finish` removes the dangling images and new buildah containers, `wt.py remove` a landed track's volumes; ask before anything else");
    Ok(())
}

/// `finish`: returns the exit status (1 when a removal did not take).
pub fn finish(out: &mut Out) -> Res<i32> {
    let r = root()?;
    let e = need_engine()?;
    let (mut removed, mut kept): (Vec<String>, Vec<String>) = (Vec::new(), Vec::new());
    let before = dangling(&e, &r)?;
    for i in &before {
        ctr(&e, &["rmi", i], true)?; // never -f: an image still in use stays
    }
    let left = dangling(&e, &r)?;
    removed.extend(
        before
            .iter()
            .filter(|i| !left.contains(i))
            .map(|i| format!("image {i}")),
    );
    kept.extend(left.iter().map(|i| format!("image {i} (in use)")));
    let base = load_baseline(&r);
    let new: Vec<External> = external(&e)?
        .into_iter()
        .filter(|x| base.as_ref().is_some_and(|b| !b.contains(&x.id)))
        .collect();
    let (mine, other) = builds_running(&r);
    if base.is_none() {
        kept.push(
            "buildah containers: no baseline (`wt.py baseline` at the start of the wave)".into(),
        );
    } else if !new.is_empty() && (!mine.is_empty() || !other.is_empty()) {
        let first = mine
            .first()
            .or_else(|| other.first())
            .cloned()
            .unwrap_or_default();
        kept.extend(
            new.iter()
                .map(|x| format!("external {} {} (a build is running: {first})", x.id, x.name)),
        );
    } else {
        for x in &new {
            ctr(&e, &["rm", "-f", &x.id], true)?;
        }
        let still: Vec<String> = external(&e)?.into_iter().map(|x| x.id).collect();
        kept.extend(
            new.iter()
                .filter(|x| still.contains(&x.id))
                .map(|x| format!("external {} {} (removal didn't take)", x.id, x.name)),
        );
        removed.extend(
            new.iter()
                .filter(|x| !still.contains(&x.id))
                .map(|x| format!("external {} {}", x.id, x.name)),
        );
    }
    for x in &removed {
        out.line(format!("removed {x}"));
    }
    for x in &kept {
        out.line(format!("left    {x}"));
    }
    if removed.is_empty() && kept.is_empty() {
        out.line("nothing to remove");
    }
    Ok(i32::from(kept.iter().any(|x| x.contains("didn't take"))))
}
