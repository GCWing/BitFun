package com.openbitfun.mobile.core.feature.session

/** Whether the peer supports the model-catalog command known by this client. */
public enum class ModelCatalogSupport {
    SUPPORTED,
    UNSUPPORTED_BY_PEER,
}

/**
 * Static model-catalog contract fact. SUPPORTED means this client knows
 * `get_model_catalog` and decodes its catalog tolerantly. UNSUPPORTED_BY_PEER is
 * reserved forward-compat for a real peer-capability signal; it is not derived
 * from a generic command rejection or malformed response, which a modern
 * desktop or a local protocol fault can also produce.
 */
public object ModelCatalogContract {
    public val commandName: String = "get_model_catalog"
    public val support: ModelCatalogSupport = ModelCatalogSupport.SUPPORTED
}
