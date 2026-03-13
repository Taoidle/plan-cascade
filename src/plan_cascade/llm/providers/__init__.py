"""
LLM Provider Implementations

Concrete implementations of LLMProvider for various LLM services:
- ClaudeProvider: Anthropic's Claude models via API
- ClaudeMaxProvider: Claude via Claude Code CLI (no API key needed)
- OpenAIProvider: OpenAI's GPT models
- DeepSeekProvider: DeepSeek's models
- GLMProvider: ZhipuAI's GLM models
- QwenProvider: Alibaba Cloud's Qwen models via DashScope
- OllamaProvider: Local models via Ollama
- NovitaProvider: Novita AI's OpenAI-compatible API
"""

from .claude import ClaudeProvider
from .claude_max import ClaudeMaxProvider
from .deepseek import DeepSeekProvider
from .glm import GLMProvider
from .novita import NovitaProvider
from .ollama import OllamaProvider
from .openai import OpenAIProvider
from .qwen import QwenProvider

__all__ = [
    "ClaudeProvider",
    "ClaudeMaxProvider",
    "DeepSeekProvider",
    "GLMProvider",
    "NovitaProvider",
    "OpenAIProvider",
    "QwenProvider",
    "OllamaProvider",
]
