# JocoHunt 등록 문구

## 이름

Kavranta

## 한 줄 소개

Codex에게 “만들고, 가져오고, 배포해줘”라고 말하세요. 환경변수는 Kavranta가 안전하게 전달합니다.

## 소개

프로젝트를 만들 때마다 `.env` 파일을 찾고, 같은 키를 복사하고, 배포 사이트에 다시 입력하는 일이 번거롭지 않으셨나요?

Kavranta는 프로젝트에 흩어진 환경변수를 한 화면에 모아주는 무료 데스크톱 앱입니다. 한곳에서 값을 입력하면 연결된 `.env.local`, `.env.development` 같은 파일에 함께 적용됩니다.

더 편한 점은 Codex와 함께 사용할 때입니다. “AUTH_SECRET 만들어줘”, “다른 프로젝트의 API 키를 여기에서도 사용해줘”, “이 변수들만 GitHub staging과 Cloudflare에 올려줘”처럼 요청할 수 있습니다. 실제 값을 채팅에 복사하지 않아도 Kavranta가 필요한 곳으로 전달하고, Codex에는 변수 이름과 작업 결과만 보여줍니다.

## 핵심 기능

- 프로젝트를 등록하면 `.env`, `.env.local`, `.dev.vars` 같은 파일을 자동으로 찾습니다.
- 같은 변수는 여러 파일에 연결하고 한 번만 입력해 함께 바꿀 수 있습니다.
- 아직 값이 없는 변수만 모아 보거나, 주석을 기준으로 그룹별로 빠르게 찾을 수 있습니다.
- Codex가 새 비밀값을 만들거나 다른 프로젝트의 값을 가져와도 원문을 채팅에 출력하지 않습니다.
- 로그인·API 테스트도 허용한 프로젝트와 변수만 사용하고 결과만 확인할 수 있습니다.
- 필요한 변수만 골라 GitHub, Cloudflare, EAS, AWS 등에 전송할 수 있습니다.
- 팀원에게는 암호화된 파일로 공유하고, 서로 다른 값은 적용 전에 직접 고를 수 있습니다.

## Codex에서 이렇게 요청할 수 있어요

> AUTH_SECRET을 새로 만들어서 `.env.local`에 넣어줘. 값은 보여주지 마.

> 다른 프로젝트에서 쓰는 GEMINI API 키를 이 프로젝트에도 연결해줘.

> staging에 필요한 변수만 GitHub와 Cloudflare에 올리고 성공 여부만 알려줘.

> 저장된 계정으로 로그인 API를 테스트하고 상태 코드만 확인해줘.

## 등록 정보 추천

- 카테고리: 개발자 도구
- 플랫폼: 데스크톱
- 가격: 오픈소스 또는 무료
- 웹사이트: https://github.com/haechan1103/kavranta
- 다운로드: https://github.com/haechan1103/kavranta/releases/latest

## 이미지 순서

1. `kavranta-main.webp`
2. `kavranta-env-editor.webp`
3. `kavranta-codex-create-reuse.webp`
4. `kavranta-codex-provider-push.webp`
5. `kavranta-codex-action-pack.webp`

앱 아이콘에는 `kavranta-app-icon.webp`를 사용합니다.

`kavranta-codex-workflow.webp`는 세부 이미지 세 장을 대신해 한 장만 올리고
싶을 때 사용하는 요약본입니다.
