# Top Codex goal prompt

Windows Terminalを第一ターゲットとするRust製terminal-native code editor「Editor」を、Microsoft Edit級の軽さと即応性を保ちながら、VS Code利用者が説明なしで使える編集・ワークスペース・LSP・診断・検索・Git体験を備えた実用品として完成させる。Requirements配下のMVP仕様、品質・安全・性能要件、ソースと対応文書の同期、テストがすべて満たされ、分散実装の成果が`devenv`へ統合されて全体検証に成功し、その同一の検証済み状態が`main`へ昇格して初めて作業完了とする。機能が表面上動くだけで、未処理のエラー経路、データ損失リスク、無効なWorkspace Trust、未同期docs、失敗または無効化されたテスト、placeholder/TODO、未統合の機能、`devenv`と`main`の不一致が残る状態は作業完了ではない。
