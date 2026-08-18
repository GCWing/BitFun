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

function featureTransitivelyReferencesOptionalDependencyOwner(
  features,
  featureName,
  depName,
  visiting = new Set(),
) {
  if (visiting.has(featureName)) {
    return false;
  }
  const feature = features.get(featureName);
  if (featureReferencesOptionalDependencyOwner(feature, depName)) {
    return true;
  }
  const nextVisiting = new Set(visiting).add(featureName);
  return Boolean(feature?.refs.some((reference) =>
    features.has(reference)
      && featureTransitivelyReferencesOptionalDependencyOwner(
        features,
        reference,
        depName,
        nextVisiting,
      )));
}

export function unexpectedDependencyOwnerFeatures(
  features,
  dependency,
  reviewedAggregateFeatures = new Set(),
) {
  return [...features.entries()].filter(
    ([featureName, feature]) => {
      if (dependency.ownerFeatures.includes(featureName)) {
        return false;
      }
      const directlyReferencesOwner = featureReferencesOptionalDependencyOwner(
        feature,
        dependency.depName,
      );
      if (reviewedAggregateFeatures.has(featureName)) {
        return directlyReferencesOwner;
      }
      return featureTransitivelyReferencesOptionalDependencyOwner(
        features,
        featureName,
        dependency.depName,
      );
    },
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
