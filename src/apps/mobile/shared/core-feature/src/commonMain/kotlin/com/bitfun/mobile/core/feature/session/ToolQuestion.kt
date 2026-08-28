package com.bitfun.mobile.core.feature.session

public data class QuestionOption public constructor(
    public val label: String,
    public val description: String?,
)

public data class ToolQuestion public constructor(
    public val index: Int,
    public val header: String,
    public val question: String,
    public val options: List<QuestionOption>,
    public val multiSelect: Boolean,
)

public sealed interface QuestionAnswerValue {
    public data class Text public constructor(
        public val text: String,
    ) : QuestionAnswerValue

    public data class Choice public constructor(
        public val values: List<String>,
    ) : QuestionAnswerValue
}

public data class QuestionAnswer public constructor(
    public val index: Int,
    public val value: QuestionAnswerValue,
)
