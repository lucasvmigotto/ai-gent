# Jira (Cloud, Server and Data Center)

An issue tracker beside any of the code platforms — most often with
Bitbucket, but also GitHub, GitLab or Azure Repos.

## Site and token — `ai-gent/jira/token`

- **Site:** `git config --global ai-gent.jira.url https://<site>.atlassian.net`
  (per repository without `--global`), or `JIRA_URL`, or a
  `url: https://…` line in the entry.
- **Jira Cloud:** an API token (id.atlassian.com → Security → API tokens;
  a token **with scopes**, read-only — `read:jira-work` — when that's all
  the work needs, plus `write:jira-work` to create, comment and
  transition), with your Atlassian email on the next line. Sent as Basic
  `email:token`.
- **Server / Data Center:** a personal access token (Profile → Personal
  Access Tokens), no email line. Sent as `Bearer`.

```bash
pass insert -m ai-gent/jira/token
# <token>
# email: me@example.com
# url: https://acme.atlassian.net
```

## CLI (`jira`, from ankitpokhrel/jira-cli)

Optional. It needs its own one-time `jira init` (server, login, project),
which the user runs; reporun hands it `JIRA_API_TOKEN` from pass.

| Need | Command (after `reporun cli jira --`) |
|---|---|
| Search | `issue list -q "project = KEY AND status != Done AND assignee = currentUser()" --plain --columns key,summary,status` |
| Read one | `issue view KEY-12 --comments 5 --plain` |
| Create | `issue create -tTask -s"<summary>" -b"<body>" --no-input` ✎ |
| Comment / transition / assign | `issue comment add KEY-12 "<text>" --no-input`, `issue move KEY-12 "In Progress"`, `issue assign KEY-12 "<user>"` ✎ |

## REST (`reporun api jira …`)

Cloud is `/rest/api/3` (rich text as Atlassian Document Format); `/rest/api/2`
takes plain-text bodies and also works on Cloud, so prefer it for
descriptions and comments. Server/Data Center is `/rest/api/2`.

| Need | Request |
|---|---|
| Search (JQL — a read) | Cloud: `GET /rest/api/3/search/jql --query 'jql=project = KEY AND statusCategory != Done ORDER BY updated DESC' --query fields=summary,status,assignee` (pages with `nextPageToken`); Server: `GET /rest/api/2/search --query 'jql=…'` (`startAt`) |
| One issue | `GET /rest/api/2/issue/KEY-12 --query fields=summary,status,description,comment` |
| Create | `POST /rest/api/2/issue` `{"fields":{"project":{"key":"KEY"},"issuetype":{"name":"Task"},"summary":"<s>","description":"<plain text>"}}` ✎ |
| Comment | `POST /rest/api/2/issue/KEY-12/comment` `{"body":"<plain text>"}` ✎ |
| Transition | `GET /rest/api/2/issue/KEY-12/transitions` (ids) → `POST …/transitions` `{"transition":{"id":"31"}}` ✎ |
| Who am I | `GET /rest/api/2/myself` |

Commits and branches link by the issue key (`KEY-12`) when the Jira and
code-platform integration is on; transitions from commit messages
(`KEY-12 #done`) need smart commits enabled.
