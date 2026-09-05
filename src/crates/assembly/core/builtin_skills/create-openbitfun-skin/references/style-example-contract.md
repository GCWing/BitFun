# Style example contract

A style example teaches the model how one visual direction chooses to use the parent Appearance contract. It is not a registry, full-surface baseline, or mandatory template.

## Required contents

Each example contains:

```text
<example-id>/
  SKILL.md
  references/
    style-playbook.md
    surface-plan.json
    default-palette.json       # when the style uses a fixed semantic palette
    palette-contract.md        # when palette validation is non-trivial
  scripts/                     # only when deterministic style-specific generation is useful
```

Do not add README, changelog, installation, or process-history files to the Skill example.

## SKILL.md responsibilities

State:

- the visual goal and suitable source material;
- the required parent workflow and references;
- the style's palette and material strategy;
- which surfaces are emphasized and which remain default;
- asset roles and crop rules, when applicable;
- style-specific build commands;
- runtime risks and inspection requirements;
- the fact that the registry remains authoritative.

Do not repeat the full package contract, validator documentation, registry maintenance workflow, or generic media implementation.

## Surface plan requirements

Use this metadata:

```json
{
  "schema": "openbitfun.appearance.style-surface-plan",
  "schemaVersion": 1,
  "scope": "example-style-selection",
  "styleId": "example-id",
  "validatedAgainstRegistryRevision": "<revision>",
  "scenes": {},
  "components": {}
}
```

The plan must be sparse and must represent deliberate style coverage. Validate every selected Part and property against the bundled registry. Never describe the plan as the current Appearance surface, and never copy it wholesale into another style.

## Script ownership

Import generic lifecycle helpers from `scripts/build_support.py` and generic media operations from `scripts/media_support.py`.

Keep these inside the example:

- semantic palette interpretation;
- materials and renderer aesthetics;
- image roles and role-specific transforms;
- crop defaults;
- surface selection;
- style-specific runtime checks;
- the thin command wrapper that combines those choices.

Use only the current command path and current record schema. Records must identify the recipe that produced them.

## Golden outputs

Importable example skins live outside this Skill. Link them conceptually through machine-readable metadata rather than copying their packages or source media into the Skill.

Golden outputs prove that a recipe can produce a real package. They do not turn one work's colors, crops, or component coverage into reusable rules.
