<div align="center">
  <img src="assets/brand/env-manager-logo-v1.png" width="104" alt="Env Manager 로고" />
  <h1>Env Manager</h1>
  <p><strong>프로젝트가 이미 사용하는 <code>.env</code>를 편집하고, 연결하고, 공유하고, 배포하세요.</strong></p>
  <p><a href="README.md">English</a> · <a href="README.ko.md">한국어</a></p>
  <p>
    <a href="https://github.com/haechan1103/env_manager/releases/latest"><img alt="최신 릴리스" src="https://img.shields.io/github/v/release/haechan1103/env_manager?style=flat-square&color=168463" /></a>
    <a href="https://github.com/haechan1103/env_manager/actions/workflows/ci.yml"><img alt="CI" src="https://img.shields.io/github/actions/workflow/status/haechan1103/env_manager/ci.yml?branch=main&style=flat-square&label=CI" /></a>
    <a href="LICENSE"><img alt="MIT 라이선스" src="https://img.shields.io/github/license/haechan1103/env_manager?style=flat-square" /></a>
    <img alt="macOS" src="https://img.shields.io/badge/platform-macOS-111111?style=flat-square&logo=apple" />
    <img alt="Windows" src="https://img.shields.io/badge/platform-Windows-0078D4?style=flat-square&logo=windows" />
  </p>
</div>

![프로젝트 개요, env 편집, Cloudflare 전송, AI 도구 연결을 보여주는 Env Manager 데모](assets/screenshots/env-manager-demo.gif)

Env Manager는 환경변수를 위한 로컬 우선 데스크톱 앱입니다. 프로젝트를 등록하면 기존 `.env`, `.env.local`, `.env.development`, 하위 앱 env 파일을 한 화면에서 관리합니다. 프로젝트의 실행 방식은 바꾸지 않고 값도 별도 전용 vault로 옮기지 않습니다.

<div align="center">
  <a href="https://github.com/haechan1103/env_manager/releases/latest"><strong>macOS·Windows용 다운로드</strong></a>
  ·
  <a href="#ai-코딩-에이전트-연결">AI 에이전트 연결</a>
  ·
  <a href="SECURITY.md">보안 모델</a>
</div>

## 이럴 때 활용하세요

| 상황 | Env Manager가 해주는 일 |
| --- | --- |
| 바이브코딩 중 AI가 API 키 입력을 요청할 때 | 채팅에 값을 붙여 넣지 않고 마스킹된 데스크톱 화면에서 직접 입력합니다. AI는 이름·그룹과 허용된 작업을 계속 다룰 수 있습니다. |
| `.env.local`, `.env.development`, 여러 앱 env가 흩어져 있을 때 | 실제 경로와 원본 형식을 유지하면서 한곳에서 발견하고 이동합니다. |
| 같은 키를 2개, 3개 이상의 파일에서 똑같이 유지해야 할 때 | 원하는 변수들만 명시적으로 연결하고 어느 파일에서든 한 번 수정해 함께 저장합니다. |
| 팀원에게 로컬 설정의 일부만 전달해야 할 때 | 전체 또는 선택한 변수만 암호화된 `age` 파일로 내보내고, 받는 프로젝트에 병합합니다. |
| 로컬 값을 배포 환경에 등록해야 할 때 | 필요한 변수만 골라 공식 로컬 CLI를 통해 GitHub Actions 또는 Cloudflare Workers로 올립니다. |
| Codex·Claude Code·Copilot이 env 구조를 수정해야 할 때 | Agent Skill과 redacted broker를 연결해 보호된 값을 일반 조회 응답에서 제외합니다. |

![Git 보호와 AI 접근 상태를 보여주는 Env Manager 프로젝트 개요](assets/screenshots/env-manager-overview.png)

## 기존 env 파일을 그대로 관리합니다

- 직접 등록한 프로젝트 안에서만 지원 env 파일을 찾습니다.
- 기존 프레임워크와 실행 명령이 사용하는 실제 파일을 편집합니다.
- 관련 없는 주석과 형식을 보존하면서 그룹·설명·변수를 정리합니다.
- 프로젝트와 env 파일에는 로컬 표시 이름을 붙일 수 있으며 실제 경로는 바꾸지 않습니다.
- 값은 기본적으로 가리고, 환경변수명 복사와 짧은 명시적 값 보기를 지원합니다.
- 변수가 많은 파일에서는 그룹 빠른 이동으로 원하는 구역으로 바로 이동합니다.
- Git ignore 누락, 이미 추적된 파일, 과거 기록, 공개 프론트엔드 변수명의 위험을 구분해 알려줍니다.
- 현재 버전은 `.env.example` 계열을 탐색에서 의도적으로 제외합니다.

![마스킹된 합성 값, 연결 파일, 그룹 빠른 이동을 보여주는 Env Manager 파일 편집기](assets/screenshots/env-manager-editor.png)

## GitHub Actions와 Cloudflare로 선택 전송

관리 중인 env 파일 하나에서 필요한 변수만 골라 배포 서비스로 보낼 수 있습니다. 이미 설치된 공식 CLI를 실행하고 값은 표준 입력으로 전달합니다. Env Manager가 서비스 토큰이나 임시 env 파일을 저장하지 않습니다.

| 서비스 | 지원 대상 | 대상 찾기 |
| --- | --- | --- |
| GitHub Actions | 저장소 또는 배포 Environment의 Secret과 설정 Variable | 가장 가까운 Git worktree와 GitHub `origin`을 기본 감지하고, `gh`로 접근 가능한 저장소·Environment를 불러오며 필요한 Environment를 직접 생성할 수 있습니다. |
| Cloudflare Workers | 기본 Worker 또는 Wrangler 환경의 Worker Secret | 가장 가까운 `wrangler.jsonc`, `wrangler.json`, `wrangler.toml`에서 Worker 이름과 설정된 `env.*` 환경을 감지합니다. |

![Wrangler를 통해 선택한 마스킹 변수를 Cloudflare Worker로 보내는 화면](assets/screenshots/env-manager-cloudflare-push.png)

이 기능은 사용자가 직접 시작하는 **단방향 전송**입니다. 원격 Secret 값을 다시 읽거나 로컬 값과 같은지 비교하지 않으며, 선택하지 않은 원격 항목을 삭제하지도 않습니다. 먼저 [`gh`](https://cli.github.com/manual/gh_secret_set) 또는 [Wrangler](https://developers.cloudflare.com/workers/wrangler/commands/#secret-bulk)를 설치하고 로그인해야 합니다.

## 전체 또는 일부만 암호화해서 공유

- **일반 ZIP:** 신뢰할 수 있는 로컬 환경에서 사용하는 평문 내보내기입니다.
- **암호화 내보내기:** 평문 중간 ZIP 없이 `age` 호환 암호화 파일을 만듭니다.
- **전체·부분 선택:** 모든 관리 파일, 특정 파일, 개별 변수만 선택할 수 있으며 연결된 변수는 함께 선택됩니다.
- **안전한 가져오기:** 없는 변수는 추가하고 받는 사람의 관련 없는 내용은 유지하며, 서로 다른 로컬 값은 적용 전에 선택합니다.
- 암호는 저장하거나 복구하지 않습니다. 파일과 암호는 서로 다른 신뢰할 수 있는 채널로 전달하세요.

![암호화된 Env Manager 공유 파일에 넣을 개별 변수를 선택하는 화면](assets/screenshots/env-manager-encrypted-share.png)

## AI 코딩 에이전트 연결

독립적으로 버전 관리되는 하나의 로컬 번들이 **Codex**, **Claude Code**, **GitHub Copilot / VS Code**를 지원합니다. 데스크톱 앱의 **AI 도구 연결**에서 지원 도구를 감지하고 Env Manager 연동을 설치할 수 있습니다.

![Codex, Claude Code, GitHub Copilot용 Env Manager 연결 화면](assets/screenshots/env-manager-ai-integrations.png)

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
Database 그룹을 만들고 DATABASE_URL 빈 변수를 추가해줘.
GPT_API_KEY를 local과 development에서 연결해줘.
```

연동 도구가 프로젝트를 임의로 등록하지는 않습니다. 데스크톱 앱에 이미 등록된 프로젝트만 broker가 허용합니다.

## AI 접근 정책

| 정책 | 에이전트 접근 |
| --- | --- |
| `protected` | 이름과 값 존재 여부만 확인하고 실제 값은 차단 |
| `unclassified` | 정책을 고르기 전까지 `protected`처럼 차단 |
| `read-write` | 명시적인 broker 값 도구가 값을 읽거나 수정할 수 있음 |

일반 구조 조회는 `read-write` 값도 반환하지 않습니다. 이미 `read-write`로 지정된 키에 전용 값 도구가 호출된 경우에만 값이 반환될 수 있습니다.

> Env Manager는 실수로 값을 노출할 가능성을 줄여주지만 운영체제 수준의 샌드박스나 운영용 Secret Manager의 대체재는 아닙니다. 값은 원래 env 파일에 남습니다. 전체 경계는 [SECURITY.md](SECURITY.md)를 확인하세요.

## 설치

[GitHub Releases](https://github.com/haechan1103/env_manager/releases/latest)에서 컴퓨터에 맞는 설치 파일을 받으세요.

- Windows 10/11 x64: `x64-setup.exe`
- Apple Silicon(M1 이상): `aarch64` DMG
- Intel Mac: `x86_64` DMG

### Windows 첫 실행

현재 Windows 설치 파일은 아직 Authenticode 서명을 사용하지 않습니다.
Microsoft Defender SmartScreen이 **Windows의 PC 보호** 경고를 표시할 수
있습니다. 공식 GitHub Release에서 받은 파일임을 확인한 경우에만
**추가 정보 → 실행**을 선택하세요. 조직에서 관리하는 컴퓨터는 서명되지
않은 앱 실행을 완전히 차단할 수도 있습니다. Windows 코드 서명은 예정되어
있습니다.

### macOS 첫 실행

현재 공개 빌드는 ad-hoc 서명을 사용하며 아직 Apple 공증을 받지 않았습니다.
따라서 이 저장소에서 받은 앱이어도 처음 실행할 때 macOS가
**“Env Manager.app”을 열 수 없음** 경고를 표시할 수 있습니다.

공식 GitHub Release에서 받은 파일임을 확인했고 실행하려는 경우:

1. 경고 창에서 **휴지통으로 이동** 대신 **완료**를 누릅니다.
2. **시스템 설정 → 개인정보 보호 및 보안**을 엽니다.
3. 아래쪽 **보안** 영역에서 Env Manager의 **확인 없이 열기**를 누릅니다.
4. 사용자 인증을 마친 뒤 마지막 확인 창에서 **열기**를 누릅니다.

**확인 없이 열기**는 실행이 차단된 뒤 약 1시간 동안 표시됩니다. 다른
경로에서 받은 파일에는 이 우회 절차를 사용하지 마세요. 자세한 내용은
[확인되지 않은 개발자의 앱을 여는 Apple 공식 안내](https://support.apple.com/ko-kr/guide/mac-help/mh40616/mac)를 참고하세요.
이 과정이 필요 없도록 Apple Developer ID 서명과 공증을 적용하는 작업은
예정되어 있습니다.

앱은 고정된 GitHub Releases 주소에서 서명된 앱 업데이트만 확인합니다. 업데이트 확인 중 프로젝트 경로, env 메타데이터, 값을 보내지 않습니다.

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

Env Manager는 초기 단계의 macOS·Windows 데스크톱 프로젝트입니다. `0.6.1`은 현재 크기를 가장 작은 단계로 두고, 네 단계의 글자 크기와 선택값 저장 기능을 추가합니다. Windows 10/11 x64 설치 파일, 로컬 파일 편집, 연결 변수, 충돌 검토가 가능한 암호화 전달, GitHub/Cloudflare 전송, 보호된 AI 에이전트 연동과 영어·한국어 UI도 그대로 지원합니다. Authenticode로 서명된 Windows 빌드, 공증된 macOS 빌드, Windows ARM64와 더 많은 언어는 다음 단계입니다.

## 커뮤니티

질문과 아이디어는 [GitHub Discussions](https://github.com/haechan1103/env_manager/discussions), 버그와 범위가 정해진 기능 요청은 [Issues](https://github.com/haechan1103/env_manager/issues)에 남겨주세요.

[Code of Conduct](CODE_OF_CONDUCT.md)를 따라주세요. 보안 문제는 [SECURITY.md](SECURITY.md)의 비공개 절차로 신고해야 합니다.

## 라이선스

[MIT](LICENSE)
