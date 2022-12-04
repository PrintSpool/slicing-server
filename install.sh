id -u slicing-worker >/dev/null 2>&1 && echo "Skipping user creation: User already exists." || sudo useradd -m slicing-worker
