"""
Loopless LangGraph Example
==========================
Demonstrates how to use Loopless to detect a repeat loop.
Requires: pip install loopless
"""
import json
import os
from loopless import Orchestrator


def main():
    config_path = os.path.join(os.path.dirname(__file__), "..", "loopless.yaml")
    orch = Orchestrator(config_path)

    # Simulate an agent calling the same tool with the same args repeatedly
    for i in range(5):
        intervention = orch.process(
            thread_id="demo",
            step=i,
            tool_name="get_revenue",
            arguments=json.dumps({"year": 2024}),
        )
        if intervention:
            print(f"Step {i}: Intervention detected!")
            if intervention.is_abort():
                print(f"  Aborted: {intervention.reason()}")
                print("  Execution halted — no more API calls burned!")
            elif intervention.is_inject_prompt():
                print(f"  Prompt injected: {intervention.prompt_content()}")
            break
        else:
            print(f"Step {i}: Tool call allowed (no intervention)")

    print(f"\nMetrics: {orch.metrics}")


if __name__ == "__main__":
    main()
