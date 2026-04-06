# OpenCode Workflow Test

This PR is used to test the workflow fix for preventing feedback loops.

The workflow should now only trigger when comments start with:
- /oc
- /opencode
- @oc
- @opencode

Comments with 'opencode' in the middle (like response links) should NOT trigger the workflow.

