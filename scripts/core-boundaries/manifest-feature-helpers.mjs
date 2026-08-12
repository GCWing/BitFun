export function featureReferencesDependency(feature, depName) {
  return Boolean(
    feature
      && feature.refs.some(
        (reference) =>
          reference === `dep:${depName}`
          || reference === depName
          || reference.startsWith(`${depName}/`),
      ),
  );
}

export function featureReferencesOptionalDependencyOwner(feature, depName) {
  return Boolean(
    featureReferencesDependency(feature, depName)
      || feature?.refs.some((reference) => reference.startsWith(`${depName}?/`)),
  );
}

export function featureReferencesFeature(feature, featureName) {
  return Boolean(feature && feature.refs.includes(featureName));
}

export function unexpectedDependencyOwnerFeatures(features, dependency) {
  return [...features.entries()].filter(
    ([featureName, feature]) =>
      featureReferencesOptionalDependencyOwner(feature, dependency.depName)
      && !dependency.ownerFeatures.includes(featureName),
  );
}

export function unexpectedReachableLocalFeatures(
  features,
  rootFeatureName,
  allowedFeatureNames,
) {
  const unexpected = [];
  const visited = new Set([rootFeatureName]);
  const pending = [{ featureName: rootFeatureName, path: [rootFeatureName] }];

  while (pending.length > 0) {
    const current = pending.shift();
    const feature = features.get(current.featureName);
    if (!feature) {
      continue;
    }
    for (const reference of feature.refs) {
      if (!features.has(reference) || visited.has(reference)) {
        continue;
      }
      visited.add(reference);
      const path = [...current.path, reference];
      if (!allowedFeatureNames.has(reference)) {
        unexpected.push({ featureName: reference, path });
      }
      pending.push({ featureName: reference, path });
    }
  }

  return unexpected;
}
