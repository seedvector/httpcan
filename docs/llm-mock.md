# AI API mock (`/llm`)

OpenAI- and Anthropic-compatible endpoints that return deterministic responses. Point any SDK's `base_url` at httpcan to test AI clients, agents, and proxies without a paid provider. No API key is checked (any value is accepted).

| Endpoint | Compatible with |
|---|---|
| `POST /llm/v1/chat/completions` | OpenAI Chat Completions (streaming supported) |
| `POST /llm/v1/responses` | OpenAI Responses API (streaming supported) |
| `POST /llm/v1/completions` | OpenAI legacy completions (text in, text out) |
| `POST /llm/v1/messages` | Anthropic Messages (streaming supported) |
| `GET /llm/v1/models` | Model list, shaped for whichever family the request looks like (an `anthropic-version` header switches it) |
| `GET /llm` | Self-describing index |

Every `/llm` response carries `X-Httpcan-Mock: true`, so a test can assert the traffic never left the mock.

## Quick examples

```bash
# OpenAI-compatible chat completions
curl -X POST https://httpcan.org/llm/v1/chat/completions \
  -H "Content-Type: application/json" \
  -d '{"model":"gpt-5.6-sol","messages":[{"role":"user","content":"Hello"}]}'

# Anthropic-compatible messages
curl -X POST https://httpcan.org/llm/v1/messages \
  -H "Content-Type: application/json" \
  -d '{"model":"claude-sonnet-5","max_tokens":64,"messages":[{"role":"user","content":"Hello"}]}'

# OpenAI-compatible Responses API
curl -X POST https://httpcan.org/llm/v1/responses \
  -H "Content-Type: application/json" \
  -d '{"model":"gpt-5.6-sol","input":"Hello"}'

# OpenAI-compatible legacy completions
curl -X POST https://httpcan.org/llm/v1/completions \
  -H "Content-Type: application/json" \
  -d '{"model":"gpt-5.6-sol","prompt":"Once upon a time"}'
```

## From an SDK

```python
from openai import OpenAI

client = OpenAI(base_url="https://httpcan.org/llm/v1", api_key="not-needed")
reply = client.chat.completions.create(
    model="gpt-5.6-sol",
    messages=[{"role": "user", "content": "Hello"}],
    extra_body={"httpcan": {"content": "Custom reply text"}},
)
print(reply.choices[0].message.content)  # Custom reply text
```

The Anthropic SDK works with `base_url="https://httpcan.org/llm"` (it appends `/v1/messages` itself).

## Scripting the response: the `httpcan` block

Every `/llm` request accepts an `httpcan` object that scripts the mock's answer the same way for every provider. It travels in the request body, so the mock stays stateless and works through any SDK via `extra_body`. The examples below assume a local instance on the default port (`httpcan`, or `docker run -p 8080:8080 …`).

```bash
# Tool calls in each provider's wire shape (message.tool_calls /
# content[]{type:"tool_use"} / output[]{type:"function_call"}), streaming
# included. tool_choice:"required" (or {"type":"any"}) synthesizes a call
# from the first declared tool when tool_calls is unset.
curl -X POST localhost:8080/llm/v1/chat/completions -H "Content-Type: application/json" \
  -d '{"messages":[{"role":"user","content":"weather?"}],"httpcan":{"tool_calls":[{"name":"get_weather","arguments":{"city":"SF"}}]}}'

# Error injection with provider-correct bodies: rate_limit (429) |
# overloaded (503, or 529 on Anthropic) | context_length (400) |
# invalid_api_key (401) | content_filter (200 with a refusal)
curl -X POST localhost:8080/llm/v1/messages -H "Content-Type: application/json" \
  -d '{"messages":[{"role":"user","content":"x"}],"httpcan":{"error":"overloaded"}}'  # -> 529

# Deterministic token usage (billing / cost-tracker tests), time-to-first-byte
# delay (delay_ms, max 10s) and stream pacing (stream_chunk_delay_ms, max 1s;
# long content collapses to at most 600 deltas so every stream finishes in
# bounded time)
curl -X POST localhost:8080/llm/v1/chat/completions -H "Content-Type: application/json" \
  -d '{"messages":[{"role":"user","content":"x"}],"httpcan":{"usage":{"prompt_tokens":10,"completion_tokens":5},"delay_ms":300}}'

# Multi-turn sequences: the step advances once per tool turn the client sends
# back (a batch of role:"tool" / tool_result / function_call_output results;
# parallel results count once). The last step repeats.
curl -X POST localhost:8080/llm/v1/chat/completions -H "Content-Type: application/json" \
  -d '{"messages":[{"role":"user","content":"weather?"}],"httpcan":{"sequence":[{"tool_calls":[{"name":"get_weather","arguments":{"city":"SF"}}]},{"content":"72F and sunny"}]}}'
```

The legacy completions API has no tools, so `tool_calls` there returns a 400 with a pointer to `/llm/v1/chat/completions`.

## Testing from chat apps (Cherry Studio, LobeChat, Open WebUI, …)

Chat UIs can't add fields to the request body, so two controls work from the prompt itself.

**1. `#mock` directives.** Type them into the chat box. `#mock` must be a standalone word in the **last user message**; the rest of its line is space-separated `key=value` directives (unknown ones are ignored):

```text
#mock error=rate_limit          → 429 provider-shaped error (also: overloaded 529/503,
                                  context_length, invalid_api_key, content_filter)
#mock delay=3000                → 3s time-to-first-byte delay
#mock usage=10x5                → fixed prompt × completion token counts
#mock tool=get_weather          → answer with one tool call; arguments are generated
                                  from the declared tool's JSON schema (required fields
                                  filled with schema-valid values), else {}
#mock tool=bash args={"command": "ls -la"}
                                → explicit arguments (a JSON object that takes the rest
                                  of the line, so put args= last)
#mock error=overloaded delay=500 → combine freely
```

Only the last user message is scanned, and only while no tool results follow it. So a directive fires once per user turn, `tool=` loops end once the tool result comes back, older messages never trigger it again, and `#mockery` doesn't count. An explicit `httpcan` block in the same request always wins over prompt directives.

**2. The `auto-tools` model.** Pick it as the model name (it's listed in `/llm/v1/models`). With tools declared (e.g. Cherry Studio's MCP servers) it calls the first declared tool. Once the client sends the tool result back, it replies `Tool result: <last result>`. You get a full visible tool loop with zero configuration.

Cherry Studio recipe: add a custom OpenAI-compatible provider (API Address `http://localhost:8080/llm/v1`, any key), fetch the model list, pick `auto-tools`, enable an MCP server, and chat. Type `#mock` directives in the message box to trigger errors, fixed usage and delays in the same conversation.

## What it does not do

Responses are deterministic placeholders. There is no real model behind them and no record/replay of real provider traffic. Use it to test how your client handles the protocol, not the quality of the answers.
