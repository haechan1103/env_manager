<div align="center">
  <img src="assets/brand/env-manager-logo-concept-v1.png" width="104" alt="Env Manager 로고" />
  <h1>Env Manager</h1>
  <p><strong>흩어진 <code>.env</code>는 한곳에서, 보호된 값은 지원 AI 에이전트에게 넘기지 않은 채 관리하세요.</strong></p>
  <p><a href="README.md">English</a> · <a href="README.ko.md">한국어</a></p>
  <p>
    <a href="https://github.com/haechan1103/env_manager/releases/latest"><img alt="최신 릴리스" src="https://img.shields.io/github/v/release/haechan1103/env_manager?style=flat-square&color=168463" /></a>
    <a href="https://github.com/haechan1103/env_manager/actions/workflows/ci.yml"><img alt="CI" src="https://img.shields.io/github/actions/workflow/status/haechan1103/env_manager/ci.yml?branch=main&style=flat-square&label=CI" /></a>
    <a href="LICENSE"><img alt="MIT 라이선스" src="https://img.shields.io/github/license/haechan1103/env_manager?style=flat-square" /></a>
    <img alt="macOS" src="https://img.shields.io/badge/platform-macOS-111111?style=flat-square&logo=apple" />
  </p>
</div>

![프로젝트 개요, env 편집, AI 도구 연결을 보여주는 Env Manager 데모](assets/screenshots/env-manager-demo.gif)

Env Manager는 프로젝트 안에 이미 존재하는 `.env` 파일을 위한 로컬 우선 데스크톱 앱입니다. 프로젝트를 등록하면 env 파일을 찾아주고, 보기 편한 화면에서 변수를 편집하고, 여러 파일의 같은 값을 명시적으로 연결할 수 있습니다.

AI 코딩 에이전트와 함께 쓸 때는 지원 연동이 보호된 값을 받지 않고도 redacted 로컬 broker를 통해 env 구조를 확인하고 수정할 수 있습니다.

<div align="center">
  <a href="https://github.com/haechan1103/env_manager/releases/latest"><strong>macOS용 다운로드</strong></a>
  ·
  <a href="#ai-코딩-에이전트-연결">AI 에이전트 연결</a>
  ·
  <a href="SECURITY.md">보안 모델</a>
</div>

## 왜 Env Manager인가요?

- **흩어진 파일을 한 화면에.** 등록한 프로젝트의 `.env`, `.env.local`, `.env.development`, 하위 앱 env 파일을 발견합니다.
- **기존 실행 방식을 유지.** 값을 별도 vault로 옮기지 않고 원래 파일을 직접 편집합니다.
- **연결된 값은 한 번에.** 같은 키를 2개 이상 파일에 연결하고 어느 곳에서든 한 번만 수정합니다.
- **읽기 쉬운 구조.** 원본 파일을 납작하게 만들지 않고 그룹·설명·변수·주석을 관리합니다.
- **기본값은 보호.** 화면에서는 값을 가리고, 일반 AI 도구 응답에서는 값을 제거합니다.
- **로컬에서 가볍게.** 등록한 프로젝트만 탐색하고 앱이 켜진 동안 발견된 env 파일만 감시합니다.

## 설치

[GitHub Releases](https://github.com/haechan1103/env_manager/releases/latest)에서 Mac에 맞는 DMG를 받으세요.

- Apple Silicon(M1 이상): `aarch64` DMG
- Intel Mac: `x86_64` DMG

현재 공개 빌드는 ad-hoc 서명을 사용합니다. 첫 실행이 차단되면 Finder에서 Env Manager를 Control-클릭하고 **열기**를 선택하세요. Apple Developer ID 공증과 Windows 지원은 다음 단계입니다.

앱은 GitHub Releases에서 업데이트 여부만 확인하며 프로젝트 경로나 환경변수 데이터는 보내지 않습니다.

## 사용 흐름

1. Env Manager에서 프로젝트 폴더를 등록합니다.
2. 발견된 env 파일과 확인할 항목을 살펴봅니다.
3. 그룹과 설명을 추가하고, AI 접근을 분류하거나 같은 변수를 연결합니다.
4. 한 번 수정하면 원본 파일 또는 명시적으로 연결한 모든 파일에 저장됩니다.

현재 버전은 `.env.example` 계열을 탐색에서 의도적으로 제외합니다.

<details>
  <summary><strong>파일 편집 화면과 AI 연동 화면 보기</strong></summary>
  <br />
  <img src="assets/screenshots/env-manager-editor.png" alt="합성 값이 가려진 Env Manager 파일 편집 화면" />
  <br /><br />
  <img src="assets/screenshots/env-manager-ai-integrations.png" alt="Env Manager AI 도구 연동 화면" />
</details>

## AI 코딩 에이전트 연결

하나의 로컬 번들이 **Codex**, **Claude Code**, **GitHub Copilot / VS Code**를 지원합니다. 데스크톱 앱의 **AI 도구 연결** 화면에서 지원 도구를 감지하고 설치할 수 있습니다.

터미널에서 직접 설치할 수도 있습니다.

<details>
  <summary><strong>Codex</strong></summary>

```bash
codex plugin marketplace add haechan1103/env_manager
codex plugin add env-manager@env-manager
```
</details>

<details>
  <summary><strong>Claude Code</strong></summary>

```bash
claude plugin marketplace add haechan1103/env_manager
claude plugin install env-manager@env-manager
```
</details>

<details>
  <summary><strong>GitHub Copilot CLI / VS Code</strong></summary>

```bash
copilot plugin marketplace add haechan1103/env_manager
copilot plugin install env-manager@env-manager
```
</details>

먼저 Env Manager 앱에 프로젝트를 등록하고 새 에이전트 세션에서 자연스럽게 요청하세요.

```text
이 프로젝트 env 구조를 값 없이 점검해줘.
GPT_API_KEY를 local과 development에서 연결해줘.
GPT 그룹을 만들고 DATABASE_URL 빈 변수를 추가해줘.
```

연동 도구가 프로젝트를 임의로 등록하지는 않습니다. 데스크톱 앱에 이미 등록된 프로젝트만 broker가 허용합니다.

## AI 접근 정책

| 정책 | 에이전트 접근 |
| --- | --- |
| `protected` | 이름과 값 존재 여부만 확인하고 실제 값은 차단 |
| `unclassified` | 정책을 고르기 전까지 `protected`처럼 차단 |
| `read-write` | 명시적인 broker 값 도구가 값을 읽거나 수정할 수 있음 |

일반 구조 조회는 `read-write` 값도 반환하지 않습니다. 이미 `read-write`로 지정된 키에 전용 값 도구가 호출된 경우에만 값이 반환됩니다.

> Env Manager는 운영체제 수준의 샌드박스나 운영용 secret manager의 대체재가 아닙니다. 값은 원래 env 파일에 남고 필요할 때 메모리에서 처리됩니다. 전체 경계와 신고 절차는 [SECURITY.md](SECURITY.md)를 확인하세요.

## 로컬 개발

Node.js, npm, Rust 1.85 이상과 [Tauri 2 사전 요구 사항](https://v2.tauri.app/start/prerequisites/)이 필요합니다.

```bash
git clone https://github.com/haechan1103/env_manager.git
cd env_manager
npm install
npm run tauri dev
```

PR을 열기 전 다음 명령을 확인하세요.

```bash
npm run check
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

테스트에는 합성 env fixture만 사용하세요. 실제 `.env*` 값은 커밋하거나 첨부하면 안 됩니다. 변경 전 [CONTRIBUTING.md](CONTRIBUTING.md)를 읽어주세요.

## 프로젝트 상태

Env Manager는 초기 단계의 macOS 프로젝트입니다. 현재 릴리스는 안정적인 로컬 파일 편집과 보호된 AI 에이전트 연동에 집중합니다. Windows 지원, macOS 공증 빌드, 더 넓은 다국어 지원은 다음 단계입니다.

## 커뮤니티

질문과 아이디어는 [GitHub Discussions](https://github.com/haechan1103/env_manager/discussions), 버그와 범위가 정해진 기능 요청은 [Issues](https://github.com/haechan1103/env_manager/issues)에 남겨주세요.

[Code of Conduct](CODE_OF_CONDUCT.md)를 따라주세요. 보안 문제는 [SECURITY.md](SECURITY.md)의 비공개 절차로 신고해야 합니다.

## 라이선스

[MIT](LICENSE)
