# Renderer contracts

List registered renderer adapters with:

```powershell
python scripts/bitfun_appearance.py contract list renderers
```

Renderer entries use this shape:

```json
{
  "renderers": {
    "<renderer-id>": {
      "version": 1,
      "settings": {}
    }
  }
}
```

## css-tokens

`settings` contains `tokens` and `background`. Query accepted token names with:

```powershell
python scripts/bitfun_appearance.py contract tokens css
```

## monaco

Accepted settings: `id`, `base`, `inherit`, `rules`, and `colors`. IDs use lowercase letters, digits, and hyphens. Supported bases are `vs`, `vs-dark`, `hc-black`, and `hc-light`.

## xterm

Accepted settings: `surfaces`, `fontWeight`, and `fontWeightBold`. `surfaces` may define `terminal` and `output` color maps.

## mermaid

Accepted settings: `mode` and `palette`. The validator reports unsupported or missing palette fields.

## generative-widget

Accepted settings: `id`, `mode`, and `vars`. Query accepted variable names with:

```powershell
python scripts/bitfun_appearance.py contract tokens widget
```

## bitfun-canvas

Accepted settings: `id`, `mode`, `bg`, `panel`, `fg`, `muted`, `border`, `accent`, `success`, `warning`, `danger`, and `info`.

Run package validation for the exact value constraints and required fields.
