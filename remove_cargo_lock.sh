#!/bin/bash
# Скрипт для удаления Cargo.lock из Git репозитория

set -e

echo "=== Удаление Cargo.lock из Git ==="

# Удаляем Lock из корня
if git ls-files --error-unmatch Cargo.lock 2>/dev/null; then
    echo "Удаляем Cargo.lock из корня..."
    git rm Cargo.lock
else
    echo "Cargo.lock в корне не найден в Git"
fi

# Удаляем Lock из programs
if git ls-files --error-unmatch programs/orbital_pulse/Cargo.lock 2>/dev/null; then
    echo "Удаляем programs/orbital_pulse/Cargo.lock..."
    git rm programs/orbital_pulse/Cargo.lock
else
    echo "Cargo.lock в programs не найден в Git"
fi

# Коммитим
echo "Коммитим изменения..."
git commit -m "Remove Cargo.lock from Git to fix CI"

# Пушим
echo "Отправляем на GitHub..."
git push

echo ""
echo "=== ГОТОВО ==="
echo "GitHub Actions перезапустится автоматически через 30-60 секунд"
echo "Проверьте: https://github.com/YYurq/Orbital_Pulse_Blockchain-Proof_of_Pose/actions"
