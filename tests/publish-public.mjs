import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { mkdtempSync, mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { after, test } from "node:test";
import { publish, rewriteHistory } from "../scripts/publish-public.mjs";

const root = mkdtempSync(join(tmpdir(), "qkmj-publish-test-"));
after(() => rmSync(root, { recursive: true, force: true }));

function git(cwd, ...args) {
  return execFileSync("git", args, { cwd, encoding: "utf8" }).trim();
}

function initRepo(path) {
  mkdirSync(path, { recursive: true });
  git(path, "init", "--initial-branch=main");
  git(path, "config", "user.name", "Publisher Test");
  git(path, "config", "user.email", "publisher-test@example.invalid");
}

function commit(cwd, message) {
  git(cwd, "add", "-A");
  git(cwd, "commit", "-m", message);
  return git(cwd, "rev-parse", "HEAD");
}

function bare(path) {
  git(root, "init", "--bare", "--initial-branch=main", path);
  return path;
}

function seedPrivate(name = "private") {
  const remote = bare(join(root, `${name}.git`));
  const work = join(root, `${name}-seed`);
  initRepo(work);
  writeFileSync(join(work, "src.txt"), "older\n");
  commit(work, "Seed public source history");
  writeFileSync(join(work, "src.txt"), "initial\n");
  mkdirSync(join(work, ".agentflow"), { recursive: true });
  writeFileSync(join(work, ".agentflow", "secret.md"), "private history\n");
  writeFileSync(join(work, "ag.json"), "private config\n");
  commit(work, "Seed private source and paths");
  git(work, "remote", "add", "origin", remote);
  git(work, "push", "-u", "origin", "main");
  const checkout = join(root, `${name}-checkout`);
  git(root, "clone", remote, checkout);
  git(checkout, "config", "user.name", "Publisher Test");
  git(checkout, "config", "user.email", "publisher-test@example.invalid");
  return { remote, checkout };
}

function seedPublic(name = "public") {
  const remote = bare(join(root, `${name}.git`));
  const work = join(root, `${name}-seed`);
  initRepo(work);
  writeFileSync(join(work, "src.txt"), "initial\n");
  const sha = commit(work, "Initial filtered snapshot");
  git(work, "remote", "add", "origin", remote);
  git(work, "push", "-u", "origin", "main");
  return { remote, work, sha };
}

test("publisher waits for public changes to reach private, then publishes a filtered child snapshot", async () => {
  const privateRepo = seedPrivate();
  const publicRepo = seedPublic();
  git(privateRepo.checkout, "config", "agentflow.public-base", publicRepo.sha);

  writeFileSync(join(publicRepo.work, "contribution.txt"), "from public\n");
  const contributionSha = commit(publicRepo.work, "Public contribution");
  git(publicRepo.work, "push", "origin", "main");
  git(publicRepo.work, "switch", "-c", "contributor/pr");
  writeFileSync(join(publicRepo.work, "pr.txt"), "pending public PR\n");
  const prSha = commit(publicRepo.work, "Pending pull request");
  git(publicRepo.work, "push", "origin", "contributor/pr");
  git(publicRepo.work, "switch", "main");

  await assert.rejects(
    publish({ repoRoot: privateRepo.checkout, privateUrl: privateRepo.remote, publicUrl: publicRepo.remote }),
    /missing from private/i,
  );
  assert.equal(git(publicRepo.work, "rev-parse", "main"), contributionSha);

  writeFileSync(join(privateRepo.checkout, "contribution.txt"), "from public\n");
  commit(privateRepo.checkout, "Sync public contribution");
  writeFileSync(join(privateRepo.checkout, "src.txt"), "private update\n");
  commit(privateRepo.checkout, "Private source update");
  git(privateRepo.checkout, "push", "origin", "main");

  const result = await publish({ repoRoot: privateRepo.checkout, privateUrl: privateRepo.remote, publicUrl: publicRepo.remote });
  assert.equal(result.published, true);
  assert.equal(git(publicRepo.work, "fetch", "origin", "main"), "");
  const publicHead = git(publicRepo.work, "rev-parse", "FETCH_HEAD");
  assert.equal(git(publicRepo.work, "show", "-s", "--format=%P", publicHead), contributionSha);
  assert.equal(git(publicRepo.work, "ls-remote", "origin", "refs/heads/contributor/pr").split(/\s+/)[0], prSha);
  assert.equal(git(publicRepo.work, "show", `${publicHead}:src.txt`), "private update");
  assert.equal(readFileSync(join(publicRepo.work, "contribution.txt"), "utf8"), "from public\n");
  assert.equal(git(publicRepo.work, "ls-tree", "-r", "--name-only", publicHead).split("\n").some((path) => path === "ag.json" || path.startsWith(".agentflow/")), false);

  const again = await publish({ repoRoot: privateRepo.checkout, privateUrl: privateRepo.remote, publicUrl: publicRepo.remote });
  assert.equal(again.published, false);
  assert.equal(git(publicRepo.work, "rev-parse", "FETCH_HEAD"), publicHead);

  git(publicRepo.work, "switch", "-c", "contributor/private-path");
  mkdirSync(join(publicRepo.work, ".agentflow"), { recursive: true });
  writeFileSync(join(publicRepo.work, ".agentflow", "leak.md"), "must remain private\n");
  commit(publicRepo.work, "Invalid public private path");
  git(publicRepo.work, "push", "origin", "contributor/private-path");
  await assert.rejects(
    publish({ repoRoot: privateRepo.checkout, privateUrl: privateRepo.remote, publicUrl: publicRepo.remote }),
    /Public history contains filtered paths/i,
  );
});

test("one-time rewrite removes private paths from every public commit", async () => {
  const privateRepo = seedPrivate("rewrite-private");
  const publicRepo = bare(join(root, "rewrite-public.git"));
  const publicWork = join(root, "rewrite-public-seed");
  initRepo(publicWork);
  writeFileSync(join(publicWork, "src.txt"), "old\n");
  mkdirSync(join(publicWork, ".agentflow"), { recursive: true });
  writeFileSync(join(publicWork, ".agentflow", "ask.md"), "old record\n");
  writeFileSync(join(publicWork, "ag.json"), "old config\n");
  commit(publicWork, "Initial public content");
  writeFileSync(join(publicWork, "src.txt"), "new\n");
  const expectedSha = commit(publicWork, "Second public commit");
  git(publicWork, "remote", "add", "origin", publicRepo);
  git(publicWork, "push", "-u", "origin", "main");

  const result = await rewriteHistory({
    repoRoot: privateRepo.checkout,
    privateUrl: privateRepo.remote,
    publicUrl: publicRepo,
    expectedPublicSha: expectedSha,
  });
  assert.equal(result.rewritten, true);

  const checkout = join(root, "rewritten-checkout");
  git(root, "clone", publicRepo, checkout);
  const commits = git(checkout, "rev-list", "main").split("\n");
  assert.equal(commits.length, 2);
  for (const sha of commits) {
    const paths = git(checkout, "ls-tree", "-r", "--name-only", sha).split("\n");
    assert.equal(paths.includes("ag.json"), false);
    assert.equal(paths.some((path) => path === ".agentflow" || path.startsWith(".agentflow/")), false);
  }
});
