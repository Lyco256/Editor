# Process bootstrap

Role: owns process-level startup and exit-code conversion. The foundation adapter provides a safe
headless vertical path; the integrated terminal adapter replaces it when terminal input is wired. No
panic is used for environment failures. Headless runtime tests cover the lifecycle contract.

