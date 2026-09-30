import { execFileSync } from "node:child_process";
import { mkdtempSync, realpathSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { pathToFileURL } from "node:url";

const PRIVATE_URL = "https://github.com/kurotanshi/qkmj-web-agentflow.git";
const PUBLIC_URL = "https://github.com/kurotanshi/qkmj-web.git";
const BASE_KEY = "agentflow.public-base";
const PRIVATE_PATHS = [".agentflow", "ag.json"];

function git(repo, args, options = {}) {
  return execFileSync("git", args, {
    cwd: repo,
    encoding: "utf8",
    stdio: ["ignore", "pipe", "pipe"],
    ...options,
  }).trim();
}

function command(args, options = {}) {
  return execFileSync(args[0], args.slice(1), {
    encoding: "utf8",
    stdio: ["ignore", "pipe", "pipe"],
    ...options,
  }).trim();
}

function assert(condition, message) {
  if (!condition) throw new Error(message);
}

function currentRepo(repoRoot, privateUrl) {
  const top = git(repoRoot, ["rev-parse", "--show-toplevel"]);
  assert(realpathSync(top) === realpathSync(repoRoot), `Run this command from the repository root: ${top}`);
  assert(git(repoRoot, ["branch", "--show-current"]) === "main", "Private canonical branch must be main.");
  assert(git(repoRoot, ["remote", "get-url", "origin"]) === privateUrl, `origin must point to private canonical: ${privateUrl}`);
  assert(git(repoRoot, ["diff", "--quiet", "HEAD", "--"]) === "", "Tracked worktree changes must be committed before publishing.");
  const head = git(repoRoot, ["rev-parse", "HEAD"]);
  const privateHead = git(repoRoot, ["ls-remote", privateUrl, "refs/heads/main"]).split(/\s+/)[0];
  assert(head && head === privateHead, "Private main must be pushed before public publication.");
  return head;
}

function publicRefs(url) {
  const refs = git(tmpdir(), ["ls-remote", "--heads", "--tags", url]);
  return refs.split("\n").filter(Boolean).map((line) => {
    const [sha, name] = line.split(/\s+/);
    return { sha, name };
  }).filter(({ name }) => !name.endsWith("^{}")).sort((a, b) => a.name.localeCompare(b.name));
}

function publicMainSha(refs) {
  const main = refs.find(({ name }) => name === "refs/heads/main");
  assert(main, "Public repository has no refs/heads/main.");
  return main.sha;
}

function assertRewriteRefs(refs) {
  assert(refs.length === 1 && refs[0].name === "refs/heads/main", "One-time rewrite requires public refs to be exactly refs/heads/main with no tags.");
}

function localPublicRef(name) {
  if (name.startsWith("refs/heads/")) return `refs/remotes/public/${name.slice("refs/heads/".length)}`;
  return `refs/tags/public/${name.slice("refs/tags/".length)}`;
}

function fetchPublicRefs(repo, url, refs) {
  const refspecs = refs.map(({ name }) => `+${name}:${localPublicRef(name)}`);
  if (refspecs.length) git(repo, ["fetch", "--no-tags", url, ...refspecs]);
}

function assertPublicRefsFiltered(repo, refs) {
  const commits = new Set();
  for (const { name } of refs) {
    const ref = localPublicRef(name);
    for (const commit of git(repo, ["rev-list", ref]).split("\n").filter(Boolean)) commits.add(commit);
  }
  for (const commit of commits) {
    const violations = pathViolations(repo, commit);
    assert(violations.length === 0, `Public history contains filtered paths in ${commit}: ${violations.join(", ")}. Remove them from public history before publishing.`);
  }
}

function clonePrivate(privateUrl, directory) {
  git(tmpdir(), ["clone", "--no-tags", "--single-branch", "--branch", "main", "--no-local", privateUrl, directory]);
}

function pathViolations(repo, commit) {
  const paths = git(repo, ["ls-tree", "-r", "--name-only", commit]).split("\n").filter(Boolean);
  return paths.filter((path) => path === ".agentflow" || path.startsWith(".agentflow/") || path === "ag.json");
}

function filteredTree(repo, privateHead) {
  const index = join(tmpdir(), `qkmj-publish-index-${process.pid}-${Date.now()}`);
  const env = { ...process.env, GIT_INDEX_FILE: index };
  try {
    command(["git", "read-tree", privateHead], { cwd: repo, env });
    command(["git", "rm", "--cached", "-r", "-q", "--ignore-unmatch", "--", ...PRIVATE_PATHS], { cwd: repo, env });
    return command(["git", "write-tree"], { cwd: repo, env });
  } finally {
    rmSync(index, { force: true });
  }
}

function assertFilteredHistory(repo) {
  for (const commit of git(repo, ["rev-list", "--all"]).split("\n").filter(Boolean)) {
    const violations = pathViolations(repo, commit);
    assert(violations.length === 0, `Filtered history still contains private paths in ${commit}: ${violations.join(", ")}`);
  }
}

function publicChanges(repo, base, head) {
  const commits = git(repo, ["rev-list", "--reverse", `${base}..${head}`]).split("\n").filter(Boolean);
  const paths = new Set();
  for (const commit of commits) {
    const violations = pathViolations(repo, commit);
    if (violations.length) {
      throw new Error(`Public history contains filtered paths in ${commit}: ${violations.join(", ")}. Remove them from public history before publishing.`);
    }
    for (const path of git(repo, ["diff-tree", "--no-commit-id", "--name-only", "-r", "-m", commit]).split("\n").filter(Boolean)) paths.add(path);
  }
  return { commits, paths: [...paths].sort() };
}

function syntheticCommit(repo, tree, parent, message) {
  const env = {
    ...process.env,
    GIT_AUTHOR_NAME: "Agentflow Publisher",
    GIT_AUTHOR_EMAIL: "agentflow-publisher@localhost",
    GIT_COMMITTER_NAME: "Agentflow Publisher",
    GIT_COMMITTER_EMAIL: "agentflow-publisher@localhost",
  };
  return command(["git", "commit-tree", tree, "-p", parent, "-m", message], { cwd: repo, env });
}

function reconcile(repo, baseline, publicHead, privateTree) {
  try {
    git(repo, ["merge-base", "--is-ancestor", baseline, publicHead]);
  } catch {
    throw new Error("Public main no longer descends from the last published baseline; inspect the public history before continuing.");
  }
  const changes = publicChanges(repo, baseline, publicHead);
  if (changes.commits.length === 0) return changes;

  const publicTree = git(repo, ["rev-parse", `${publicHead}^{tree}`]);
  const publicSide = syntheticCommit(repo, publicTree, baseline, "Temporary public side for reconciliation");
  const privateSide = syntheticCommit(repo, privateTree, baseline, "Temporary private side for reconciliation");
  let mergedTree;
  try {
    mergedTree = command(["git", "merge-tree", "--write-tree", publicSide, privateSide], { cwd: repo }).split("\n")[0];
  } catch {
    throw new Error(`Public and private changes conflict. Public commits: ${changes.commits.join(", ")}; paths: ${changes.paths.join(", ") || "(none)"}. Sync the public changes into private main, resolve conflicts, and retry.`);
  }
  if (mergedTree !== privateTree) {
    throw new Error(`Public changes are missing from private. Public commits: ${changes.commits.join(", ")}; paths: ${changes.paths.join(", ") || "(none)"}. Sync these changes into private main, then retry.`);
  }
  return changes;
}

function updateBaseline(repoRoot, sha) {
  git(repoRoot, ["config", "--local", BASE_KEY, sha]);
}

export async function publish({ repoRoot = process.cwd(), privateUrl = PRIVATE_URL, publicUrl = PUBLIC_URL } = {}) {
  const privateHead = currentRepo(repoRoot, privateUrl);
  let baseline = "";
  try {
    baseline = git(repoRoot, ["config", "--local", "--get", BASE_KEY]);
  } catch {
    // The baseline is set by the approved one-time rewrite.
  }
  assert(/^[0-9a-f]{40,64}$/.test(baseline), `No valid ${BASE_KEY} is recorded. Complete the one-time history rewrite first.`);
  const beforeRefs = publicRefs(publicUrl);
  const expectedPublicHead = publicMainSha(beforeRefs);
  const temp = mkdtempSync(join(tmpdir(), "qkmj-publish-"));
  try {
    const clone = join(temp, "private");
    clonePrivate(privateUrl, clone);
    fetchPublicRefs(clone, publicUrl, beforeRefs);
    const afterRefs = publicRefs(publicUrl);
    assert(JSON.stringify(beforeRefs) === JSON.stringify(afterRefs), "Public refs changed during preflight; retry after inspecting the latest refs.");
    assertPublicRefsFiltered(clone, afterRefs);
    const publicHead = git(clone, ["rev-parse", "refs/remotes/public/main"]);
    assert(publicHead === expectedPublicHead, "Public main changed during preflight; retry after inspecting the latest public commit.");
    const privateTree = filteredTree(clone, privateHead);
    const { commits, paths } = reconcile(clone, baseline, publicHead, privateTree);
    const publicTree = git(clone, ["rev-parse", `${publicHead}^{tree}`]);
    if (privateTree === publicTree) {
      updateBaseline(repoRoot, publicHead);
      return { published: false, sha: publicHead, reconciledCommits: commits.length };
    }

    const snapshot = syntheticCommit(clone, privateTree, publicHead, "Publish filtered private snapshot");
    git(clone, ["push", publicUrl, `${snapshot}:refs/heads/main`]);
    const pushedHead = publicMainSha(publicRefs(publicUrl));
    assert(pushedHead === snapshot, "Public main did not land on the prepared snapshot; baseline was left unchanged.");
    updateBaseline(repoRoot, snapshot);
    return { published: true, sha: snapshot, reconciledCommits: commits.length, paths };
  } finally {
    rmSync(temp, { recursive: true, force: true });
  }
}

export async function rewriteHistory({ repoRoot = process.cwd(), privateUrl = PRIVATE_URL, publicUrl = PUBLIC_URL, expectedPublicSha } = {}) {
  const privateHead = currentRepo(repoRoot, privateUrl);
  assert(/^[0-9a-f]{40,64}$/.test(expectedPublicSha ?? ""), "Supply the exact expected public main SHA.");
  const refs = publicRefs(publicUrl);
  assertRewriteRefs(refs);
  assert(publicMainSha(refs) === expectedPublicSha, "Public main differs from the expected SHA; stop and inspect before rewriting.");
  assert(git(repoRoot, ["ls-tree", "-d", "--name-only", privateHead, ".agentflow"]) === ".agentflow", "Private main must contain the .agentflow records before public rewrite.");
  assert(git(repoRoot, ["ls-tree", "--name-only", privateHead, "ag.json"]) === "ag.json", "Private main must retain root ag.json before public rewrite.");
  command(["git", "filter-repo", "--version"]);
  const temp = mkdtempSync(join(tmpdir(), "qkmj-rewrite-"));
  try {
    const clone = join(temp, "private");
    clonePrivate(privateUrl, clone);
    command(["git", "filter-repo", "--force", "--path", ".agentflow", "--path", "ag.json", "--invert-paths"], { cwd: clone });
    assertFilteredHistory(clone);
    const candidate = git(clone, ["rev-parse", "refs/heads/main"]);
    const prePushRefs = publicRefs(publicUrl);
    assertRewriteRefs(prePushRefs);
    assert(publicMainSha(prePushRefs) === expectedPublicSha, "Public main changed before rewrite; no ref was pushed.");
    git(clone, ["push", `--force-with-lease=refs/heads/main:${expectedPublicSha}`, publicUrl, `${candidate}:refs/heads/main`]);
    const afterRefs = publicRefs(publicUrl);
    assertRewriteRefs(afterRefs);
    assert(publicMainSha(afterRefs) === candidate, `Remote public main differs from filtered candidate ${candidate}; expected old SHA was ${expectedPublicSha}.`);
    updateBaseline(repoRoot, candidate);
    return { rewritten: true, sha: candidate, previousSha: expectedPublicSha };
  } finally {
    rmSync(temp, { recursive: true, force: true });
  }
}

function parseArgs(args) {
  if (args.length === 0) return { mode: "publish" };
  if (args.length === 2 && args[0] === "--rewrite") return { mode: "rewrite", expectedPublicSha: args[1] };
  throw new Error("Usage: node scripts/publish-public.mjs [--rewrite <expected-public-main-sha>]");
}

if (process.argv[1] && import.meta.url === pathToFileURL(resolve(process.argv[1])).href) {
  try {
    const args = parseArgs(process.argv.slice(2));
    const result = args.mode === "rewrite" ? await rewriteHistory(args) : await publish();
    console.log(args.mode === "rewrite" ? `Rewrote public main to ${result.sha}` : result.published ? `Published ${result.sha}` : `No public update needed (${result.sha})`);
    if (result.reconciledCommits) console.log(`Included ${result.reconciledCommits} synced public commit(s).`);
  } catch (error) {
    console.error(error.message);
    process.exitCode = 1;
  }
}
