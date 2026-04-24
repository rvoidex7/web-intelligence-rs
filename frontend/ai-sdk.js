// Type definitions for the Chrome Built-in AI API
// Supports both legacy (text session) and new (language model) APIs
/**
 * Implementation for the built-in Chrome Nano model.
 */
export class ChromeNanoProvider {
    model = null;
    async initialize(options) {
        if (!window.ai) {
            throw new Error("Window AI API not supported.");
        }
        // Try new API
        if (window.ai.languageModel) {
            try {
                this.model = await window.ai.languageModel.create(options);
                return;
            }
            catch (e) {
                console.warn("Failed to create languageModel, falling back to legacy...", e);
            }
        }
        // Try legacy API
        if (window.ai.createTextSession) {
            this.model = await window.ai.createTextSession(options);
            return;
        }
        throw new Error("No compatible AI API found.");
    }
    async generate(prompt) {
        if (!this.model)
            throw new Error("Model not initialized");
        return this.model.prompt(prompt);
    }
    async *stream(prompt) {
        if (!this.model)
            throw new Error("Model not initialized");
        const stream = this.model.promptStreaming(prompt);
        const reader = stream.getReader();
        try {
            while (true) {
                const { done, value } = await reader.read();
                if (done)
                    break;
                yield value;
            }
        }
        finally {
            reader.releaseLock();
        }
    }
    destroy() {
        if (this.model) {
            this.model.destroy();
            this.model = null;
        }
    }
    registerTool(tool, options) {
        if (window.navigator?.modelContext?.registerTool) {
            window.navigator.modelContext.registerTool(tool, options);
        }
        else {
            console.warn("window.navigator.modelContext.registerTool is not supported by the current browser.");
        }
    }
    unregisterTool(toolName) {
        if (window.navigator?.modelContext?.unregisterTool) {
            window.navigator.modelContext.unregisterTool(toolName);
        }
        else {
            console.warn("window.navigator.modelContext.unregisterTool is not supported by the current browser.");
        }
    }
}
/**
 * Implementation for OpenAI Cloud Provider.
 */
export class OpenAIProvider {
    apiKey;
    modelName;
    constructor(apiKey, modelName = "gpt-3.5-turbo") {
        this.apiKey = apiKey;
        this.modelName = modelName;
    }
    async initialize(options) {
        // No heavy init needed for REST API, but could validate key.
        if (!this.apiKey) {
            throw new Error("OpenAI API Key is missing.");
        }
    }
    async generate(prompt) {
        const response = await fetch("https://api.openai.com/v1/chat/completions", {
            method: "POST",
            headers: {
                "Content-Type": "application/json",
                "Authorization": `Bearer ${this.apiKey}`
            },
            body: JSON.stringify({
                model: this.modelName,
                messages: [{ role: "user", content: prompt }],
                stream: false
            })
        });
        if (!response.ok) {
            throw new Error(`OpenAI API Error: ${response.statusText}`);
        }
        const data = await response.json();
        return data.choices[0]?.message?.content || "";
    }
    async *stream(prompt) {
        const response = await fetch("https://api.openai.com/v1/chat/completions", {
            method: "POST",
            headers: {
                "Content-Type": "application/json",
                "Authorization": `Bearer ${this.apiKey}`
            },
            body: JSON.stringify({
                model: this.modelName,
                messages: [{ role: "user", content: prompt }],
                stream: true
            })
        });
        if (!response.ok) {
            throw new Error(`OpenAI API Error: ${response.statusText}`);
        }
        if (!response.body)
            throw new Error("No response body");
        const reader = response.body.getReader();
        const decoder = new TextDecoder("utf-8");
        let buffer = "";
        try {
            while (true) {
                const { done, value } = await reader.read();
                if (done)
                    break;
                const chunk = decoder.decode(value, { stream: true });
                buffer += chunk;
                const lines = buffer.split("\n");
                // Keep the last line in the buffer as it might be incomplete
                buffer = lines.pop() || "";
                for (const line of lines) {
                    const trimmedLine = line.trim();
                    if (trimmedLine === "data: [DONE]")
                        return;
                    if (trimmedLine.startsWith("data: ")) {
                        const jsonStr = trimmedLine.replace("data: ", "");
                        try {
                            const json = JSON.parse(jsonStr);
                            const content = json.choices[0]?.delta?.content;
                            if (content)
                                yield content;
                        }
                        catch (e) {
                            console.error("Error parsing stream chunk", e);
                        }
                    }
                }
            }
        }
        finally {
            reader.releaseLock();
        }
    }
    destroy() {
        // Nothing to clean up
    }
    registerTool(tool, options) {
        console.warn("registerTool is not yet implemented for OpenAIProvider.");
    }
    unregisterTool(toolName) {
        console.warn("unregisterTool is not yet implemented for OpenAIProvider.");
    }
}
export class AIClient {
    provider = null;
    config;
    constructor(config) {
        this.config = config || { strategy: 'local' };
    }
    /**
     * Initializes the AI Client based on the selected strategy.
     * Can also be called to re-initialize or switch strategies.
     *
     * Returns true if successful.
     * If initialization fails completely (e.g., no provider available), it throws an error object.
     */
    async init(strategyOverride) {
        const strategy = strategyOverride || this.config.strategy || 'local';
        console.log(`Initializing AI Client with strategy: ${strategy}`);
        try {
            if (strategy === 'local') {
                await this.initLocal();
            }
            else if (strategy === 'cloud') {
                await this.initCloud();
            }
            else if (strategy === 'hybrid') {
                try {
                    await this.initLocal();
                }
                catch (e) {
                    console.log("Local initialization failed in Hybrid mode, switching to Cloud.", e);
                    await this.initCloud();
                }
            }
            else {
                throw { status: 'error', reason: 'unknown_strategy', message: `Unknown strategy: ${strategy}` };
            }
        }
        catch (e) {
            // Propagate if it's already our structured error
            if (e && e.status === 'error') {
                throw e;
            }
            // Otherwise, wrap it if it's a critical initialization failure
            // The user requested: { status: 'error', reason: 'hardware_insufficient' }
            // We map internal errors to this format.
            const reason = this.mapErrorToReason(e);
            throw { status: 'error', reason: reason, message: e.message || e.toString() };
        }
    }
    mapErrorToReason(e) {
        const msg = (e.message || "").toLowerCase();
        if (msg.includes("hardware insufficient") || msg.includes("not available")) {
            return "hardware_insufficient";
        }
        if (msg.includes("openai api key")) {
            return "configuration_missing";
        }
        return "initialization_failed";
    }
    async initLocal() {
        // Check capabilities first
        if (typeof window.ai !== 'undefined' && window.ai.languageModel) {
            const caps = await window.ai.languageModel.capabilities();
            if (caps.available === 'no') {
                throw new Error("Local AI hardware insufficient or not available.");
            }
        }
        else if (typeof window.ai !== 'undefined' && window.ai.canCreateTextSession) {
            const status = await window.ai.canCreateTextSession();
            if (status === 'no')
                throw new Error("Local AI legacy API reported not available.");
        }
        else {
            // If window.ai is completely missing
            throw new Error("Local AI API missing.");
        }
        const provider = new ChromeNanoProvider();
        await provider.initialize(this.config.modelOptions);
        this.provider = provider;
    }
    async initCloud() {
        if (!this.config.openaiKey) {
            throw new Error("OpenAI API Key required for Cloud/Hybrid strategy.");
        }
        const provider = new OpenAIProvider(this.config.openaiKey);
        await provider.initialize(this.config.modelOptions);
        this.provider = provider;
    }
    async generate(prompt) {
        if (!this.provider) {
            // Auto-init if not done? Or throw?
            // Prompt implies explicit init, but let's be safe.
            throw new Error("AI Client not initialized. Call init() first.");
        }
        return this.provider.generate(prompt);
    }
    async *stream(prompt) {
        if (!this.provider) {
            throw new Error("AI Client not initialized. Call init() first.");
        }
        yield* this.provider.stream(prompt);
    }
    destroy() {
        if (this.provider) {
            this.provider.destroy();
            this.provider = null;
        }
    }
    registerTool(tool, options) {
        if (!this.provider) {
            throw new Error("AI Client not initialized. Call init() first.");
        }
        if (this.provider.registerTool) {
            this.provider.registerTool(tool, options);
        }
        else {
            console.warn("registerTool is not supported by the current provider.");
        }
    }
    unregisterTool(toolName) {
        if (!this.provider) {
            throw new Error("AI Client not initialized. Call init() first.");
        }
        if (this.provider.unregisterTool) {
            this.provider.unregisterTool(toolName);
        }
        else {
            console.warn("unregisterTool is not supported by the current provider.");
        }
    }
}
