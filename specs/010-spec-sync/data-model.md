Reconstructed by project:introspec on 2026-10-18 from 384c771

# Data model — specsync

| Entity | Fields | Evidence |
|---|---|---|
| Feature | number, name (directory), title (first `# ` line of spec.md), statuses (README row), state | `plugins/repo/scripts/specsync.py:294-306` |
| Story | n, title, priority, text | `specsync.py:63-113` |
| Task | id (`T\d+[a-z]?`), done, story, layer (Frontend/Backend/QA), text | `specsync.py:116-138` |
| Item | key (`feature`, `US<n>`), kind, title, state ∈ {todo, doing, review, done}, parent, body, body_html, hash (sha1[:10] of title+body), action, existing | `specsync.py:306-354` |
| Tracker record | key, ref (`#12`, `PROJ-3`…), id, state, hash, url — one row in `tracker.md` | `specsync.py:195-227` |

## State machine — item

```
todo ──(a task checked)──▶ doing ──(all tasks checked)──▶ review ──(feature Verified)──▶ done
```

Feature state from README: Planned → todo; In progress, Partial → doing; Implemented → review; Verified → done; least advanced side wins [OBSERVED: specsync.py:32-34,175-192].
