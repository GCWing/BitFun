import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";
import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { AskUser } from "../dist/flow-chat.js";

const questions = [
  {
    customOption: {
      description: "Provide custom text input",
      inputLabel: "Custom version",
      label: "Other",
      placeholder: "Enter a version",
      value: "other",
    },
    id: "version",
    options: [
      {
        description: "Latest Beta pre-release — newer and has passed basic testing",
        label: "v0.2.19-beta.1 (Recommended)",
        value: "beta",
      },
      {
        description: "The stable release marked as Latest on GitHub — the most stable one",
        label: "v0.2.18",
        value: "stable",
      },
    ],
    prompt: "BitFun has three versions — which would you like me to pull?",
    selectionMode: "single",
  },
];

test("AskUser renders the answered disclosure and native selection semantics", () => {
  const markup = renderToStaticMarkup(createElement(AskUser, {
    answers: { version: ["beta"] },
    expanded: true,
    questions,
    state: "completed",
    summaryDetail: "Choose version: v0.2.19-beta.1 (Recommended)",
    summaryLabel: "1 question answered",
  }));

  assert.match(markup, /data-bf-component="ask-user"/);
  assert.match(markup, /data-bf-state="completed"/);
  assert.match(markup, /data-bf-expanded="true"/);
  assert.match(markup, /<button[^>]+aria-expanded="true"/);
  assert.match(markup, /data-bf-part="summary-label">1 question answered/);
  assert.match(markup, /data-bf-part="summary-detail">Choose version:/);
  assert.match(markup, /<fieldset[^>]+disabled=""/);
  assert.match(markup, /type="radio"[^>]+checked=""/);
  assert.match(markup, /lucide-disc2/);
  assert.match(markup, /data-bf-part="description"/);
});

test("AskUser renders a controlled custom answer with an accessible text field", () => {
  const markup = renderToStaticMarkup(createElement(AskUser, {
    answers: { version: ["other"] },
    customAnswers: { version: "v0.2.17" },
    onAnswersChange: () => undefined,
    onCustomAnswerChange: () => undefined,
    questions,
    state: "asking",
    submitLabel: "Submit",
  }));

  assert.match(markup, /data-custom="true" data-selected="true"/);
  assert.match(markup, /data-bf-part="custom-input"/);
  assert.match(markup, /aria-label="Custom version"/);
  assert.match(markup, /value="v0.2.17"/);
  assert.match(markup, /data-bf-part="submit"/);
});

test("AskUser styles use public semantic and component geometry tokens", async () => {
  const styles = await readFile(new URL("../dist/styles.css", import.meta.url), "utf8");

  assert.match(styles, /--bf-control-ask-user-header-height/);
  assert.match(styles, /--bf-control-ask-user-option-padding-block/);
  assert.match(styles, /--bf-control-ask-user-question-options-gap/);
  assert.match(styles, /--bf-control-ask-user-description-max-width/);
  assert.match(styles, /--bf-color-surface-subtle/);
  assert.match(styles, /--bf-color-action-neutral-content/);
  assert.match(styles, /--bf-color-content-muted/);
  assert.match(styles, /--bf-color-status-success-content/);
  assert.match(styles, /--bf-line-height-tight/);
  assert.match(
    styles,
    /circle:nth-of-type\(2\)\s*\{\s*fill:\s*currentColor/,
  );
  assert.doesNotMatch(styles, /#[0-9a-f]{3,8}\b|rgba?\(/i);
});
