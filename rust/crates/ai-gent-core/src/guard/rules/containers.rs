//! Port of `plugins/devcontainer/hooks/guard.sh`.
use super::{any, re};
use crate::guard::Decision;
use crate::guard::reasons::decide;
use crate::guard::segment::View;

const PREFIX: &str = r"^[[:space:]]*((sudo([[:space:]]+-[^[:space:]]+([[:space:]]+[^\-[:space:]][^[:space:]]*)?)*|env|exec|nice|do|then|else|xargs([[:space:]]+-[^[:space:]]+)*|timeout[[:space:]]+[0-9smh]+|[A-Za-z_][A-Za-z0-9_]*=[^[:space:]]*)[[:space:]]+)*";
const ENGINE: &str = r"(([^[:space:]]*/)?(podman|docker|nerdctl)|\$\{?CONTAINER_ENGINE\}?)";
const BUILDAH: &str = r"([^[:space:]]*/)?buildah";
const WORDS: &str = r"([[:space:]]+[^[:space:]]+)*";

/// Evaluate the containers guard on a shell command.
#[must_use]
pub fn evaluate(v: &View) -> Decision {
    let s = &v.segments;
    if any(
        s,
        &re(&format!(
            r"{PREFIX}({ENGINE}|{BUILDAH}){WORDS}[[:space:]]+prune([[:space:]]|$)"
        )),
    ) {
        return decide("containers.prune", &[]);
    }
    let removal = format!(
        r"{PREFIX}({ENGINE}[[:space:]]+((container|image|volume)[[:space:]]+)?(rm|rmi)|{BUILDAH}[[:space:]]+(rm|rmi))"
    );
    if any(
        s,
        &re(&format!(
            r"{removal}{WORDS}[[:space:]]+(--all|-[a-zA-Z]*a[a-zA-Z]*)([[:space:]]|$)"
        )),
    ) || any(
        s,
        &re(&format!(
            r"{PREFIX}{BUILDAH}[[:space:]]+rmi{WORDS}[[:space:]]+(--prune|-p)([[:space:]]|$)"
        )),
    ) {
        return decide("containers.all", &[]);
    }
    let listing = format!(
        r"{PREFIX}{ENGINE}[[:space:]]+((container|image|volume)[[:space:]]+)?(ps|ls|list|images){WORDS}[[:space:]]+(--quiet|-[a-zA-Z]*q[a-zA-Z]*)([[:space:]]|$)"
    );
    if any(s, &re(&format!(r"{removal}([[:space:]]|$)")))
        && any(s, &re(&listing))
        && !re(r"(label|name|id)=").is_match(&v.raw)
    {
        return decide("containers.listing", &[]);
    }
    Decision::none()
}
