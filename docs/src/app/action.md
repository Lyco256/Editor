# Application actions

Role: normalized user intent entering the state machine. Actions contain data only and never perform
I/O. Input adapters create them; `AppState` consumes them. Trust changes and effect requests are
explicit so policy tests can observe authorization decisions.

