const CHANNELS = {
  stable: {
    primaryUpdaterEndpoint:
      'https://github.com/GCWing/BitFun/releases/latest/download/latest.json',
    fallbackUpdaterEndpoint: 'https://openbitfun.com/release/latest.json',
    githubChannelTag: null,
  },
  beta: {
    primaryUpdaterEndpoint:
      'https://github.com/GCWing/BitFun/releases/download/channel-beta/latest.json',
    fallbackUpdaterEndpoint: 'https://openbitfun.com/release/beta/latest.json',
    githubChannelTag: 'channel-beta',
  },
  nightly: {
    primaryUpdaterEndpoint:
      'https://github.com/GCWing/BitFun/releases/download/nightly/latest.json',
    fallbackUpdaterEndpoint: 'https://openbitfun.com/release/nightly/latest.json',
    githubChannelTag: 'nightly',
  },
};

export function resolveReleaseChannel(value = 'stable') {
  const channel = String(value || 'stable').trim().toLowerCase();
  const config = CHANNELS[channel];
  if (!config) {
    throw new Error(
      `Unsupported release channel "${value}". Expected one of: ${Object.keys(CHANNELS).join(', ')}`,
    );
  }
  return { channel, ...config };
}

export function validateReleaseVersion(channelValue, versionValue) {
  const { channel } = resolveReleaseChannel(channelValue);
  const version = String(versionValue || '').trim();
  const stablePattern = /^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)$/;
  const betaPattern =
    /^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)-beta\.([1-9]\d*)$/;
  const nightlyPattern =
    /^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)-nightly\.\d{8}$/;
  const valid = channel === 'stable'
    ? stablePattern.test(version)
    : channel === 'beta'
      ? betaPattern.test(version)
      : nightlyPattern.test(version);
  if (!valid) {
    const example = channel === 'stable'
      ? '0.2.18'
      : channel === 'beta'
        ? '0.2.18-beta.1'
        : '0.2.18-nightly.20260811';
    throw new Error(
      `Version "${versionValue}" is invalid for the ${channel} channel. Expected a version like ${example}`,
    );
  }
  return version;
}

export function compareReleaseVersions(leftValue, rightValue) {
  const left = parseComparableVersion(leftValue);
  const right = parseComparableVersion(rightValue);
  for (let index = 0; index < 3; index += 1) {
    if (left.core[index] !== right.core[index]) {
      return left.core[index] < right.core[index] ? -1 : 1;
    }
  }
  if (left.beta === right.beta) return 0;
  if (left.beta === null) return 1;
  if (right.beta === null) return -1;
  return left.beta < right.beta ? -1 : 1;
}

function parseComparableVersion(value) {
  const match = /^(\d+)\.(\d+)\.(\d+)(?:-beta\.([1-9]\d*))?$/.exec(String(value));
  if (!match) {
    throw new Error(`Unsupported channel version: ${value}`);
  }
  return {
    core: match.slice(1, 4).map(Number),
    beta: match[4] === undefined ? null : Number(match[4]),
  };
}
