#[derive(Debug, Clone, PartialEq)]
pub enum RepoSource {
    MyRepos,
    OrgRepos(String),
    Starred,
    AllAccessible,
}
