# Findings: feature-qwen-sdk

## async-dashscope SDK (v0.12.0) Migration Findings

### Successfully Migrated

1. **Client construction**: `DashScopeClient::new().with_api_key()` and `DashScopeClient::with_config()` for custom base URLs via `ConfigBuilder`.

2. **Message building**: Uses serde JSON construction + deserialization for Input, since the SDK's `param::Message` enum is not publicly exported. This approach handles all message types including assistant messages with `tool_calls`.

3. **Tool/function definitions**: `FunctionCallBuilder` + `FunctionBuilder` work well for constructing tool definitions. `FunctionParameters` requires serde deserialization since the builder has private fields.

4. **Streaming**: `client.generation().call_stream(param)` returns `GenerationOutputStream` (a `Pin<Box<dyn Stream<Item = Result<GenerationOutput, DashScopeError>>>>`). Each chunk is a structured `GenerationOutput` with parsed `choices`, `message`, `reasoning_content`, and `tool_calls` -- no SSE parsing needed.

5. **Thinking support**: `ParametersBuilder` natively supports `.enable_thinking(true)` and `.thinking_budget(usize)`, matching the DashScope API for Qwen3 models.

6. **Result format**: `ParametersBuilder::result_format("message")` sets the response format.

7. **Parallel tool calls**: `ParametersBuilder::parallel_tool_calls(true)` is supported.

### SDK Limitations (Parameters Not Supported)

The following parameters are NOT available in the SDK's `Parameters` struct:

| Parameter | Impact | Workaround |
|-----------|--------|------------|
| `temperature` | Cannot control generation randomness | DashScope uses server-side defaults (typically 0.7-1.0) |
| `max_tokens` | Cannot limit output length | DashScope uses model-specific defaults |
| `tool_choice` | Cannot force tool usage (`"required"`) | DashScope defaults to `"auto"` which is acceptable; `"required"` was only used for non-thinking models |
| `reasoning_tokens` in Usage | Cannot report thinking token usage | SDK's `Usage` struct lacks this field |

### SDK Design Issues

1. **Private param types**: The `param::Message` enum, `param::ToolCall`, and `param::Function` structs are not publicly re-exported from the generation module. Only the builders (`MessageBuilder`, `AssistantMessageBuilder`, etc.) are exported. This makes it impossible to construct `Vec<param::Message>` directly for `InputBuilder::messages()` when assistant messages contain `tool_calls`. **Workaround**: Construct Input as JSON and deserialize via serde.

2. **`Input` type not re-exported**: The `Input` struct from param is not in the public API, but `InputBuilder` is. The `build()` method returns `Result<Input, _>` so the type can be inferred. **Workaround**: Use serde deserialization to construct Input directly.

3. **Builder `strip_option` pattern**: All builder methods use `#[builder(setter(into, strip_option))]`, so `.stream()` takes `bool` (not `Option<bool>`), `.tool_call_id()` takes `String` (not `Option<String>`), etc.

### API Endpoint Change

The SDK uses DashScope's **native API endpoint** (`/api/v1/services/aigc/text-generation/generation`) rather than the OpenAI-compatible endpoint (`/compatible-mode/v1/chat/completions`). Both endpoints support the same models and features; the native endpoint uses a slightly different request/response format that the SDK handles internally.

### Streaming Adapter Impact

The `QwenAdapter` in `services::streaming::adapters::qwen.rs` (SSE line parser) is no longer used by `QwenProvider::stream_message()` since the SDK returns structured `GenerationOutput` chunks. The adapter remains available for the `AdapterFactory` and any external SSE-based streaming scenarios.
