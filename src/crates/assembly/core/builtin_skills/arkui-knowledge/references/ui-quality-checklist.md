# ArkUI UI Quality Checklist

Use this checklist before finalizing ArkUI UI changes.

## Requirement visibility

- Required labels, button text, tab labels, card titles, and dialog text appear on the target screen.
- Text is not hidden by overlays, zero-size containers, off-screen placement, or low contrast.
- The first screen remains reachable from the app launch path.

## Interaction

- Required buttons use `Button` when the requirement asks for `Button`.
- Click handlers update visible state, switch tabs, open dialogs, navigate, or show the requested response.
- Cancel and close actions do not perform extra business actions.
- Required click targets remain large enough and visually clear.

## Layout quality

- New UI follows nearby spacing, color, typography, density, and component style.
- Layout nesting is only deep enough to express the UI.
- Dynamic text, lists, and tab content have stable width/height constraints where needed.
- New elements do not overlap existing banners, cards, bottom bars, or system safe areas.

## State and rendering

- UI-driving state uses the current component's decorator family.
- Lists and grids render stable keys.
- Conditional UI still leaves required content reachable.
- Dialog, toast, and navigation code follows existing project patterns.

## Change scope

- Only files required by the UI task are modified.
- Existing business flow, page registration, resource naming, and navigation architecture are preserved.
- No state-management migration, navigation rewrite, or broad refactor is introduced without an explicit request.
