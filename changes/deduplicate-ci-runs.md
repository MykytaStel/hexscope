### Changed

- **Avoid duplicate full CI and dependency-advisory runs for same-repository pull requests.** Branch pushes still run the checks; fork pull requests still run them in the base repository.
- **Run dependency advisory scans when manifests change** as well as when lockfiles change. The weekly and manual scans remain available.
