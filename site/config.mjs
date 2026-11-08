// The one place the site names its GitHub repository. build.mjs uses it for every
// link to GitHub and writes it on <html data-repo>, where src/site.js reads it to ask
// the GitHub API for the latest release. If the repository is renamed, change it here.
export const REPO = "uncrayon/slope-field-vizualiser";

// The branch that links to repository files (CONTRIBUTING.md, docs/contracts.md) point at.
export const BRANCH = "main";
