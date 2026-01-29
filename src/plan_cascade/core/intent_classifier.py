"""
Intent Classifier for Plan Cascade REPL

Classifies user input into different intents:
- CHAT: General conversation, questions, discussions
- TASK: Execute a development task (create, modify, fix code)
- QUERY: Information query about the project or codebase

Uses a 3-tier approach:
1. Rule-based heuristics (fast, zero cost)
2. LLM classification (when rules are uncertain)
3. User confirmation (when LLM is also uncertain)
"""

import re
from dataclasses import dataclass
from enum import Enum
from typing import TYPE_CHECKING, Any

if TYPE_CHECKING:
    from ..llm.base import LLMProvider


class Intent(Enum):
    """User intent types."""
    CHAT = "chat"           # General conversation
    TASK = "task"           # Execute a development task
    QUERY = "query"         # Query information about project
    UNCLEAR = "unclear"     # Cannot determine


@dataclass
class IntentResult:
    """Result of intent classification."""
    intent: Intent
    confidence: float  # 0.0 - 1.0
    reasoning: str
    suggested_mode: str = "simple"  # simple or expert

    def is_confident(self, threshold: float = 0.7) -> bool:
        """Check if confidence is above threshold."""
        return self.confidence >= threshold


class IntentClassifier:
    """
    Classifies user intent using rules + LLM + user confirmation.
    """

    # Task-related patterns (high confidence for TASK)
    TASK_PATTERNS = [
        # Chinese patterns
        (r"帮我(实现|创建|添加|修改|删除|修复|重构|优化)", 0.9),
        (r"(实现|创建|添加|修改|删除|修复|重构|优化).{0,20}(功能|特性|模块|组件|接口|API)", 0.85),
        (r"写一个", 0.8),
        (r"改一下", 0.8),
        (r"把.{0,30}(改成|换成|替换为)", 0.85),
        (r"(新增|增加).{0,20}(功能|特性|字段|属性|方法)", 0.85),
        # English patterns
        (r"(implement|create|add|modify|delete|fix|refactor|optimize)\s+", 0.85),
        (r"(build|make|write)\s+(a|an|the)\s+", 0.8),
        (r"(change|update|replace)\s+.{0,30}\s+(to|with)", 0.8),
        (r"please\s+(implement|create|add|fix|build)", 0.85),
        # Imperative patterns
        (r"^(给|让|请).{0,10}(加|改|删|写)", 0.8),
    ]

    # Query-related patterns (high confidence for QUERY)
    QUERY_PATTERNS = [
        # Chinese patterns
        (r"(是什么|是啥|什么是)", 0.85),
        (r"(为什么|为啥|怎么会)", 0.85),
        (r"(在哪|哪里|哪个文件)", 0.85),
        (r"(有没有|有多少|几个)", 0.8),
        (r"(怎么用|如何使用|用法)", 0.8),
        (r"(解释|说明|介绍)一下", 0.8),
        (r"(分析|看看|检查).{0,20}(结构|代码|项目)", 0.75),
        # English patterns
        (r"(what|where|which|how|why)\s+(is|are|does|do|did|can|should)", 0.85),
        (r"(explain|describe|tell me about)", 0.85),
        (r"(show|list|find)\s+(me\s+)?(the|all|any)", 0.75),
        (r"\?$", 0.6),  # Ends with question mark
    ]

    # Chat-related patterns (high confidence for CHAT)
    CHAT_PATTERNS = [
        # Chinese patterns
        (r"^(你好|嗨|hi|hello)", 0.9),
        (r"(谢谢|感谢|多谢)", 0.85),
        (r"(你觉得|你认为|你的看法)", 0.8),
        (r"(好的|可以|行|没问题)", 0.85),
        (r"(聊聊|讨论|谈谈)", 0.75),
        # English patterns
        (r"^(hi|hello|hey|thanks|thank you)", 0.9),
        (r"(what do you think|your opinion|your thoughts)", 0.8),
        (r"(sounds good|okay|alright|got it)", 0.85),
        (r"(let's discuss|can we talk about)", 0.75),
    ]

    # Context-dependent patterns (need conversation history)
    CONTEXT_PATTERNS = [
        # References to previous conversation
        (r"(上面|前面|刚才|之前)(说的|提到的|那个)", "context_reference"),
        (r"(第一个|第二个|最后一个)(建议|方案|选项)", "context_reference"),
        (r"(这个|那个|它)", "context_reference"),
        (r"(the first|the second|the last)\s+(one|suggestion|option)", "context_reference"),
        (r"(this|that|it)\b", "context_reference"),
    ]

    # Expert mode indicators (suggest expert mode for these)
    EXPERT_INDICATORS = [
        r"(复杂|大型|完整|系统性)",
        r"(multiple|complex|comprehensive|full|complete)",
        r"(架构|设计|规划|重构)",
        r"(architecture|design|plan|refactor)",
        r"(多个文件|多模块|跨模块)",
    ]

    LLM_CLASSIFICATION_PROMPT = """Analyze the user's intent from their message.

## User Message
{message}

## Conversation Context
{context}

## Task
Classify the intent into one of:
- "task": User wants to execute a development task (create, modify, fix, implement something)
- "query": User wants information or analysis (what is, where is, explain, analyze)
- "chat": General conversation, greetings, acknowledgments, discussions

Return JSON:
```json
{{
    "intent": "task" | "query" | "chat",
    "confidence": 0.0-1.0,
    "reasoning": "brief explanation",
    "suggested_mode": "simple" | "expert"
}}
```

Guidelines:
- If user references previous conversation and asks to "implement" or "do" something, it's likely "task"
- If user is asking questions about the codebase, it's "query"
- If the task seems complex (multiple features, architecture changes), suggest "expert" mode
- Return ONLY the JSON, no additional text."""

    def __init__(self, llm: "LLMProvider | None" = None):
        """
        Initialize the intent classifier.

        Args:
            llm: Optional LLM provider for uncertain cases
        """
        self.llm = llm

    async def classify(
        self,
        message: str,
        conversation_history: list[dict[str, str]] | None = None,
        confidence_threshold: float = 0.7
    ) -> IntentResult:
        """
        Classify user intent using rules, then LLM if uncertain.

        Args:
            message: User's message
            conversation_history: Previous conversation for context
            confidence_threshold: Threshold for confident classification

        Returns:
            IntentResult with intent and confidence
        """
        # Step 1: Rule-based classification
        result = self._classify_with_rules(message, conversation_history)

        if result.is_confident(confidence_threshold):
            return result

        # Step 2: LLM classification (if available and rules uncertain)
        if self.llm and result.confidence < confidence_threshold:
            try:
                llm_result = await self._classify_with_llm(message, conversation_history)
                if llm_result.is_confident(confidence_threshold):
                    return llm_result
                # If LLM is also uncertain, return with UNCLEAR intent
                if llm_result.confidence > result.confidence:
                    return llm_result
            except Exception:
                pass  # Fall back to rule result

        # Step 3: Return uncertain result (caller should ask user)
        if result.confidence < 0.5:
            result.intent = Intent.UNCLEAR
        return result

    def _classify_with_rules(
        self,
        message: str,
        conversation_history: list[dict[str, str]] | None = None
    ) -> IntentResult:
        """
        Classify using rule-based heuristics.

        Args:
            message: User's message
            conversation_history: Previous conversation

        Returns:
            IntentResult from rules
        """
        message_lower = message.lower()

        # Check for context references
        has_context_ref = any(
            re.search(pattern, message, re.IGNORECASE)
            for pattern, _ in self.CONTEXT_PATTERNS
        )

        # Score each intent
        task_score = self._match_patterns(message, self.TASK_PATTERNS)
        query_score = self._match_patterns(message, self.QUERY_PATTERNS)
        chat_score = self._match_patterns(message, self.CHAT_PATTERNS)

        # Boost task score if there's context reference and action words
        if has_context_ref and conversation_history:
            action_words = ["实现", "做", "开始", "implement", "do", "start", "begin"]
            if any(word in message_lower for word in action_words):
                task_score = max(task_score, 0.75)

        # Determine intent
        scores = {
            Intent.TASK: task_score,
            Intent.QUERY: query_score,
            Intent.CHAT: chat_score,
        }

        best_intent = max(scores, key=scores.get)
        best_score = scores[best_intent]

        # Check for expert mode indicators
        suggested_mode = "simple"
        if best_intent == Intent.TASK:
            if any(re.search(p, message, re.IGNORECASE) for p in self.EXPERT_INDICATORS):
                suggested_mode = "expert"

        # Determine reasoning
        if best_score >= 0.7:
            reasoning = f"High confidence match for {best_intent.value} patterns"
        elif best_score >= 0.5:
            reasoning = f"Moderate match for {best_intent.value} patterns"
        else:
            reasoning = "No strong pattern match, uncertain"

        return IntentResult(
            intent=best_intent if best_score >= 0.3 else Intent.UNCLEAR,
            confidence=best_score,
            reasoning=reasoning,
            suggested_mode=suggested_mode,
        )

    def _match_patterns(
        self,
        message: str,
        patterns: list[tuple[str, float]]
    ) -> float:
        """
        Match message against patterns and return highest score.

        Args:
            message: Message to match
            patterns: List of (pattern, score) tuples

        Returns:
            Highest matching score
        """
        max_score = 0.0
        for pattern, score in patterns:
            if re.search(pattern, message, re.IGNORECASE):
                max_score = max(max_score, score)
        return max_score

    async def _classify_with_llm(
        self,
        message: str,
        conversation_history: list[dict[str, str]] | None = None
    ) -> IntentResult:
        """
        Classify using LLM.

        Args:
            message: User's message
            conversation_history: Previous conversation

        Returns:
            IntentResult from LLM
        """
        # Build context from history
        context = "No previous conversation."
        if conversation_history:
            context_parts = []
            for msg in conversation_history[-6:]:  # Last 6 messages
                role = "User" if msg["role"] == "user" else "Assistant"
                content = msg["content"][:200] + "..." if len(msg["content"]) > 200 else msg["content"]
                context_parts.append(f"{role}: {content}")
            context = "\n".join(context_parts)

        prompt = self.LLM_CLASSIFICATION_PROMPT.format(
            message=message,
            context=context
        )

        response = await self.llm.complete([{"role": "user", "content": prompt}])

        # Parse response
        import json
        try:
            # Extract JSON from response
            json_match = re.search(r'\{[\s\S]*\}', response.content)
            if json_match:
                data = json.loads(json_match.group())

                intent_map = {
                    "task": Intent.TASK,
                    "query": Intent.QUERY,
                    "chat": Intent.CHAT,
                }
                intent = intent_map.get(data.get("intent", "chat"), Intent.CHAT)

                return IntentResult(
                    intent=intent,
                    confidence=float(data.get("confidence", 0.5)),
                    reasoning=data.get("reasoning", "LLM classification"),
                    suggested_mode=data.get("suggested_mode", "simple"),
                )
        except (json.JSONDecodeError, ValueError):
            pass

        # Fallback
        return IntentResult(
            intent=Intent.UNCLEAR,
            confidence=0.3,
            reasoning="Failed to parse LLM response",
            suggested_mode="simple",
        )


def get_intent_choices() -> list[dict[str, str]]:
    """Get intent choices for user confirmation."""
    return [
        {
            "value": "task",
            "label": "Execute Task",
            "description": "Implement, create, modify, or fix something"
        },
        {
            "value": "query",
            "label": "Query Info",
            "description": "Ask questions, analyze, or get information"
        },
        {
            "value": "chat",
            "label": "Just Chat",
            "description": "General discussion or conversation"
        },
    ]
