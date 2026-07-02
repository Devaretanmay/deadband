"""
Loopless OpenAI Agents Example
==============================
Demonstrates wrapping tools with Loopless interception.
Requires: pip install loopless
"""
import json
import os
from loopless import Orchestrator


def search_function(query: str) -> str:
    return f"Results for: {query}"


def main():
    config_path = os.path.join(os.path.dirname(__file__), "..", "loopless.yaml")
    orch = Orchestrator(config_path)

    print("Loopless + OpenAI Agents SDK demo:\n")

    for i in range(4):
        intervention = orch.process(
            thread_id="openai-agent",
            step=i,
            tool_name=search_function.__name__,
            arguments=json.dumps({"query": "latest news"}),
        )
        if intervention:
            print(f"Step {i}: Intercepted — {intervention}")
            if intervention.is_abort():
                print("  Would abort agent execution here")
                break
        else:
            result = search_function("latest news")
            print(f"Step {i}: Allowed → {result}")


if __name__ == "__main__":
    main()
