"""
Loopless CrewAI Example
=======================
Demonstrates how to use Loopless with CrewAI-style flows.
Requires: pip install loopless
"""
import json
import os
from loopless import Orchestrator


def main():
    config_path = os.path.join(os.path.dirname(__file__), "..", "loopless.yaml")
    orch = Orchestrator(config_path)

    print("Loopless + CrewAI demo:")
    print("(Simulated — no CrewAI dependency needed)\n")

    for i in range(4):
        intervention = orch.process(
            thread_id="crewai-flow",
            step=i,
            tool_name="search_database",
            arguments=json.dumps({"query": "SELECT * FROM users"}),
        )
        if intervention:
            print(f"Step {i}: Intervention — {intervention}")
            if intervention.is_abort():
                break
        else:
            print(f"Step {i}: Tool call passed through")


if __name__ == "__main__":
    main()
