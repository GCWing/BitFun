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

export function featureReferencesFeature(feature, featureName) {
  return Boolean(feature && feature.refs.includes(featureName));
}

export function unexpectedDependencyOwnerFeatures(features, dependency) {
  return [...features.entries()].filter(
    ([featureName, feature]) =>
      featureReferencesDependency(feature, dependency.depName)
      && !dependency.ownerFeatures.includes(featureName),
  );
}
