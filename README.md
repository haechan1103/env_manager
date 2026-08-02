# Env Manager

프로젝트에 흩어진 `.env` 파일을 한곳에서 발견하고, 정리하고, 안전하게
수정하는 로컬 데스크톱 앱입니다.

값을 별도 vault로 옮기지 않습니다. 기존 프로젝트가 사용하던 env 파일을
그대로 유지하면서 구조, 설명, 연결 상태와 Codex 접근 정책을 관리합니다.

> 현재 상태: macOS용 V1 preview
>
> Windows 지원, 서명, 공증, 자동 업데이트는 아직 제공하지 않습니다.

## 주요 기능

- 사용자가 등록한 프로젝트만 탐색
- `.env`, `.env.local`, `.env.dev`, `.env.development` 등 env 파일 자동 발견
- `.env.example`을 포함한 example 변형 제외
- 실제 값은 기본적으로 가리고 존재 여부만 표시
- `# @group GPT` 형식의 그룹 생성·이름 변경·변수 이동과 일반 주석 기반 설명 지원
- 기존 파일의 순서, 주석, 줄바꿈과 알 수 없는 구문을 최대한 보존
- 같은 변수를 2개 이상의 파일에 명시적으로 연결하거나 개별 해제
- 연결된 어느 파일에서 입력해도 모든 멤버에 함께 저장
- 같은 이름이지만 연결되지 않은 변수는 별도 상태로 표시하고 연결을 제안
- Codex가 원문 전체를 읽지 않고 redacted broker를 통해 작업하도록 지원

## 동작 방식

1. Env Manager에서 프로젝트 폴더를 등록합니다.
2. 앱을 열거나 새로고침할 때 지원하는 env 파일을 발견합니다.
3. 파일별 변수, 그룹, 설명과 보호 상태를 확인합니다.
4. 앱에서 저장하면 실제 env 파일에 반영됩니다.

앱을 켜 둔 동안에는 이미 발견한 파일만 가볍게 감시합니다. 프로젝트 전체를
항상 재귀 탐색하거나 백그라운드 데몬으로 동작하지 않습니다.

## 시작하기

### 요구 사항

- macOS
- Node.js와 npm
- Rust 1.85 이상
- [Tauri 2 시스템 요구 사항](https://v2.tauri.app/start/prerequisites/)

```bash
git clone https://github.com/haechan1103/env_manager.git
cd env_manager
npm install
npm run tauri dev
```

배포용 macOS 앱을 만들려면 다음 명령을 실행합니다.

```bash
npm run tauri build
```

현재 기본 번들 대상은 `.app`입니다. 공개 배포 전에 별도의 코드 서명과 공증
설정이 필요합니다.

## Codex 연동

Codex 플러그인은 자연어로 env 정리, 분류, 연결 작업을 요청할 때 전용
Skill과 로컬 MCP broker를 사용합니다.

먼저 이 저장소에서 broker 실행 파일을 설치합니다.

```bash
cargo install --path crates/env-broker --locked
```

그다음 저장소를 Codex 마켓플레이스로 추가하고 플러그인을 설치합니다.

```bash
codex plugin marketplace add haechan1103/env_manager
codex plugin add env-manager@env-manager
```

설치 후 새 Codex 대화를 시작하세요. 예를 들면 다음처럼 요청할 수 있습니다.

```text
이 프로젝트 env 구조를 값 없이 점검해줘.
GPT_API_KEY를 local과 development에서 연결해줘.
기존 env 주석을 관리 형식으로 정리할 계획을 만들어줘.
GPT 그룹을 만들고 DATABASE_URL 빈 변수를 추가해줘.
```

플러그인이 프로젝트를 임의로 등록하지는 않습니다. Env Manager 앱에서 먼저
등록된 프로젝트만 broker가 승인하며, 앱이 현재 등록 상태를 해제하면 더 이상
Codex에서도 접근할 수 없습니다.

## 값과 접근 정책

변수마다 Codex 접근 정책을 지정할 수 있습니다.

| 정책 | 동작 |
| --- | --- |
| `protected` | 이름과 값 존재 여부만 확인하고 실제 값 접근은 차단 |
| `unclassified` | 분류 전까지 `protected`와 동일하게 차단 |
| `read-write` | 명시적인 broker 도구를 통한 값 읽기·수정 허용 |

일반 구조 조회는 `read-write` 값도 반환하지 않습니다. 실제 값 읽기는 해당
변수가 이미 허용되어 있고, 명시적인 도구가 호출된 경우에만 가능합니다.
보호된 값의 직접 입력과 자유로운 교체는 데스크톱 앱에서 수행합니다.

### 현재 보안 한계

이 프로젝트는 완전한 secret vault가 아닙니다. 값은 원래 env 파일에 남고,
앱과 broker가 필요한 순간 프로세스 메모리에서 처리합니다.

broker는 등록된 프로젝트와 변수 정책을 검사하고 결과를 가리지만, 현재
macOS Codex 파일 권한만으로 모든 우회 쓰기를 차단했다는 보장은 아직
완료되지 않았습니다. 플러그인은 preview로 사용하고, 중요한 운영 비밀에는
별도의 secret manager와 운영체제 수준 보호를 함께 사용하세요.

## 개발 확인

```bash
npm run check
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

데스크톱 흐름과 번들까지 확인하려면 다음 명령도 실행할 수 있습니다.

```bash
npm run test:e2e
npm run tauri build
```

테스트용 env fixture에는 합성 값만 사용합니다. 실제 프로젝트의 `.env*`는
기본적으로 Git에서 제외됩니다.

## 라이선스

[MIT](LICENSE)
