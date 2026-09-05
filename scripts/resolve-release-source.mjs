import { spawnSync } from 'node:child_process';
import { pathToFileURL } from 'node:url';

export function resolveReleaseSource({ checkoutRef, releaseTag, cwd = process.cwd() }) {
  if (!checkoutRef || checkoutRef.startsWith('-') || !releaseTag) {
    throw new Error('A checkout ref and release tag are required.');
  }

  const git = (...args) => spawnSync('git', args, {
    cwd,
    encoding: 'utf8',
    windowsHide: true,
  });
  const resolveCommit = (ref) => {
    const result = git('rev-parse', '--verify', '--quiet', '--end-of-options', `${ref}^{commit}`);
    return result.status === 0 ? result.stdout.trim() : null;
  };

  let checkoutSha = resolveCommit(checkoutRef);
  if (!checkoutSha) {
    // Full branch history does not include a SHA left behind by a history
    // rewrite, or an arbitrary branch name in a detached Actions checkout.
    const fetched = git('fetch', '--no-tags', '--', 'origin', checkoutRef);
    if (fetched.status !== 0) {
      throw new Error(
        `Cannot fetch checkout ref ${checkoutRef} from origin. Start a new Desktop Package run `
        + 'from a current workflow branch and choose an available commit, branch, or tag.\n'
        + (fetched.stderr || fetched.error?.message || ''),
      );
    }
    checkoutSha = resolveCommit('FETCH_HEAD');
    if (!checkoutSha) {
      throw new Error(`Checkout ref ${checkoutRef} did not resolve to a commit.`);
    }
  }

  const tagSha = resolveCommit(`refs/tags/${releaseTag}`);
  if (tagSha && tagSha !== checkoutSha) {
    throw new Error(
      `Existing tag ${releaseTag} points to ${tagSha}, not requested commit ${checkoutSha}. `
      + 'Build the existing tag, or choose a new release tag for the requested commit.',
    );
  }
  return checkoutSha;
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  try {
    console.log(resolveReleaseSource({
      checkoutRef: process.argv[2],
      releaseTag: process.argv[3],
    }));
  } catch (error) {
    console.error(error.message);
    process.exitCode = 1;
  }
}
