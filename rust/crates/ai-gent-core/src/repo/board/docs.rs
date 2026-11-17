//! Every GraphQL document the board adapters send, named (contract §3, §4.2). Constants with
//! variables: a value from the server or the person never lands inside a document. The classification
//! test (FR-042) proves each one is read by `graphql::is_read_body`.

/// GitHub: the Projects v2 boards of a user.
pub const BOARDS_OF_USER: (&str, &str) = (
    "BoardsOfUser",
    "query BoardsOfUser($login: String!, $after: String) { user(login: $login) { projectsV2(first: 50, after: $after) { nodes { id number title url closed } pageInfo { hasNextPage endCursor } } } }",
);

/// GitHub: the Projects v2 boards of an organization.
pub const BOARDS_OF_ORG: (&str, &str) = (
    "BoardsOfOrg",
    "query BoardsOfOrg($login: String!, $after: String) { organization(login: $login) { projectsV2(first: 50, after: $after) { nodes { id number title url closed } pageInfo { hasNextPage endCursor } } } }",
);

/// GitHub: one user board and its status field.
pub const BOARD_OF_USER: (&str, &str) = (
    "BoardOfUser",
    "query BoardOfUser($login: String!, $number: Int!, $field: String!) { user(login: $login) { projectV2(number: $number) { id title url closed field(name: $field) { ... on ProjectV2FieldCommon { id name dataType } ... on ProjectV2SingleSelectField { options { id name } } } } } }",
);

/// GitHub: one organization board and its status field.
pub const BOARD_OF_ORG: (&str, &str) = (
    "BoardOfOrg",
    "query BoardOfOrg($login: String!, $number: Int!, $field: String!) { organization(login: $login) { projectV2(number: $number) { id title url closed field(name: $field) { ... on ProjectV2FieldCommon { id name dataType } ... on ProjectV2SingleSelectField { options { id name } } } } } }",
);

/// GitHub: the fields of a board (when the asked field is missing or not single-select).
pub const BOARD_FIELDS: (&str, &str) = (
    "BoardFields",
    "query BoardFields($project: ID!) { node(id: $project) { ... on ProjectV2 { fields(first: 50) { nodes { ... on ProjectV2FieldCommon { id name dataType } } } } } }",
);

/// GitHub: the items of a board with their status value.
pub const BOARD_ITEMS: (&str, &str) = (
    "BoardItems",
    "query BoardItems($project: ID!, $field: String!, $after: String) { node(id: $project) { ... on ProjectV2 { items(first: 100, after: $after) { nodes { id content { __typename ... on Issue { number url title state } ... on PullRequest { number url title state } ... on DraftIssue { title } } fieldValueByName(name: $field) { ... on ProjectV2ItemFieldSingleSelectValue { name optionId } } } pageInfo { hasNextPage endCursor } } } } }",
);

/// GitLab status mode (fixtures only, every name unverified): the project and its allowed statuses.
pub const GITLAB_STATUS_FIELDS: (&str, &str) = (
    "BoardStatusFields",
    "query BoardStatusFields($fullPath: ID!) { project(fullPath: $fullPath) { id workItemTypes { nodes { name widgetDefinitions { ... on WorkItemWidgetDefinitionStatus { allowedStatuses { id name } } } } } } }",
);

/// GitLab status mode (fixtures only, unverified): the work items of a project with their status.
pub const GITLAB_STATUS_ITEMS: (&str, &str) = (
    "BoardStatusItems",
    "query BoardStatusItems($fullPath: ID!, $after: String) { project(fullPath: $fullPath) { workItems(first: 100, after: $after) { nodes { id iid title webUrl state widgets { ... on WorkItemWidgetStatus { status { id name } } } } pageInfo { hasNextPage endCursor } } } }",
);

/// GitLab status mode: the read-only introspection for G30-21, to check the names above on a real instance.
pub const GITLAB_STATUS_INTROSPECT: (&str, &str) = (
    "BoardStatusIntrospect",
    "query BoardStatusIntrospect { widget: __type(name: \"WorkItemWidgetStatus\") { name fields { name type { name kind ofType { name kind } } } } definition: __type(name: \"WorkItemWidgetDefinitionStatus\") { name fields { name type { name kind ofType { name kind } } } } workItem: __type(name: \"WorkItem\") { fields { name } } }",
);

/// GitHub: where an issue or pull request sits on boards (the item ids and the column value).
pub const ITEM_ON_BOARD: (&str, &str) = (
    "ItemOnBoard",
    "query ItemOnBoard($url: URI!, $field: String!, $after: String) { resource(url: $url) { __typename ... on Issue { id number url title state projectItems(first: 50, after: $after) { nodes { id project { id } fieldValueByName(name: $field) { ... on ProjectV2ItemFieldSingleSelectValue { name optionId } } } pageInfo { hasNextPage endCursor } } } ... on PullRequest { id number url title state projectItems(first: 50, after: $after) { nodes { id project { id } fieldValueByName(name: $field) { ... on ProjectV2ItemFieldSingleSelectValue { name optionId } } } pageInfo { hasNextPage endCursor } } } } }",
);

/// GitLab status mode (fixtures only, unverified): one work item of a project with its status.
pub const GITLAB_STATUS_LOCATE: (&str, &str) = (
    "BoardStatusLocate",
    "query BoardStatusLocate($fullPath: ID!, $iid: String!) { project(fullPath: $fullPath) { workItems(iids: [$iid]) { nodes { id iid title webUrl state widgets { ... on WorkItemWidgetStatus { status { id name } } } } } } }",
);

/// GitHub write: add an issue or pull request to a board.
pub const BOARD_ADD: (&str, &str) = (
    "BoardAdd",
    "mutation BoardAdd($project: ID!, $content: ID!) { addProjectV2ItemById(input: {projectId: $project, contentId: $content}) { item { id } } }",
);

/// GitHub write: set the single-select value of an item.
pub const BOARD_SET: (&str, &str) = (
    "BoardSet",
    "mutation BoardSet($project: ID!, $item: ID!, $field: ID!, $option: String!) { updateProjectV2ItemFieldValue(input: {projectId: $project, itemId: $item, fieldId: $field, value: {singleSelectOptionId: $option}}) { projectV2Item { id } } }",
);

/// GitLab status mode write (fixtures only, every name unverified): set a work item's status.
pub const BOARD_STATUS: (&str, &str) = (
    "BoardStatus",
    "mutation BoardStatus($item: WorkItemID!, $status: WorkItemsStatusesStatusID!) { workItemUpdate(input: {id: $item, statusWidget: {status: $status}}) { workItem { id } errors } }",
);

/// Every read document, for the classification test and nothing else.
pub const READS: [(&str, &str); 11] = [
    BOARDS_OF_USER,
    BOARDS_OF_ORG,
    BOARD_OF_USER,
    BOARD_OF_ORG,
    BOARD_FIELDS,
    BOARD_ITEMS,
    GITLAB_STATUS_FIELDS,
    GITLAB_STATUS_ITEMS,
    GITLAB_STATUS_INTROSPECT,
    ITEM_ON_BOARD,
    GITLAB_STATUS_LOCATE,
];

/// Every write document, for the classification test and nothing else.
pub const WRITES: [(&str, &str); 3] = [BOARD_ADD, BOARD_SET, BOARD_STATUS];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::repo::api::api_is_write;
    use crate::repo::graphql::is_read_body;
    use crate::tokens::pyjson::{J, dumps, obj};

    fn body(name: &str, doc: &str) -> String {
        dumps(
            &obj(vec![
                ("query", J::s(doc)),
                ("variables", obj(vec![])),
                ("operationName", J::s(name)),
            ]),
            false,
            None,
        )
    }

    #[test]
    fn every_read_document_is_a_read() {
        for (name, doc) in READS {
            assert!(doc.starts_with(&format!("query {name}")), "{name}");
            assert!(!doc.contains("mutation"), "{name}");
            let b = body(name, doc);
            assert!(is_read_body(Some(b.as_bytes())), "{name} is not a read");
            assert!(
                !api_is_write("github", "POST", "/graphql", Some(b.as_bytes()), &[]),
                "{name}: github"
            );
            assert!(
                !api_is_write("gitlab", "POST", "/api/graphql", Some(b.as_bytes()), &[]),
                "{name}: gitlab"
            );
        }
    }

    #[test]
    fn every_write_document_is_a_write() {
        for (name, doc) in WRITES {
            assert!(doc.starts_with(&format!("mutation {name}")), "{name}");
            let b = body(name, doc);
            assert!(!is_read_body(Some(b.as_bytes())), "{name} reads as a read");
            for (platform, path) in [("github", "/graphql"), ("gitlab", "/api/graphql")] {
                assert!(
                    api_is_write(platform, "POST", path, Some(b.as_bytes()), &[]),
                    "{name}: {platform}"
                );
            }
        }
    }

    #[test]
    fn every_planned_gitlab_put_is_a_write() {
        for path in ["/projects/g%2Fp/issues/3", "/projects/7/issues/12"] {
            assert!(api_is_write(
                "gitlab",
                "PUT",
                path,
                Some(b"{\"add_labels\":\"x\"}"),
                &[]
            ));
        }
    }

    #[test]
    fn a_mutation_in_the_same_envelope_is_a_write() {
        let b = body("BoardSet", "mutation BoardSet { x }");
        assert!(!is_read_body(Some(b.as_bytes())));
        assert!(api_is_write(
            "github",
            "POST",
            "/graphql",
            Some(b.as_bytes()),
            &[]
        ));
    }

    #[test]
    fn a_document_in_the_url_makes_the_post_a_write() {
        let b = body(BOARD_ITEMS.0, BOARD_ITEMS.1);
        assert!(api_is_write(
            "github",
            "POST",
            "/graphql?query=x",
            Some(b.as_bytes()),
            &[]
        ));
    }
}
