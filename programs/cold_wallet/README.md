# Cold wallet program

기존 `solana_contract`와 다른 Program ID로 운영하는 SOL 예치·출금 프로그램입니다.
기존 프로그램의 코드, PDA 데이터, 예치금은 변경하지 않습니다.

## Program ID

`3qzzUeR3njJ2q398SAR4vZLW8cBdc3LKwBhWR9HPdGYa`

로컬 프로그램 키페어: `target/deploy/cold_wallet-keypair.json`.
이 파일은 Git에서 제외됩니다. 배포 시 같은 주소를 유지하려면 별도로 보관해야 합니다.
현재는 파일만 생성했으며 온체인 배포는 하지 않았습니다.

## Instructions

- `deposit(commitment: [u8; 32], amount: u64, ipfs_uri: String)`
  - 최소 1,000,000 lamports (0.001 SOL).
  - IPFS 주소는 `ipfs://<CID>` 또는 `ipfs://<CID>/<path>` 형식으로 전달합니다.
  - 주소는 UTF-8 기준 최대 200바이트이며 공백을 허용하지 않습니다.
  - 접두사와 길이만 검사합니다. CID 유효성, 파일 존재 여부, 파일 내용은 검증하지 않습니다.
  - 예치 시 저장하며 이후 수정하는 instruction은 없습니다.
- `withdraw_zkp(nonce: [u8; 32], amount: u64)`
  - 기존 프로그램의 Ed25519 서명 검증 및 부분·전액 출금 방식을 유지합니다.
  - 첫 번째 transaction instruction은 Ed25519 검증 instruction이어야 합니다.
  - 서명 메시지: `SHA256(commitment || recipient || nonce || amount.to_le_bytes())`.
  - nonce 사용 이력을 저장하지 않으므로 같은 서명을 반복 사용할 수 있습니다.
  - 요청액이 남은 예치금 이상이면 rent 예치금까지 반환하며 계정 잔액을 0으로 만듭니다.

## Account

PDA seeds: `[b"commitment", commitment]` + **cold_wallet Program ID**.
같은 commitment를 사용해도 기존 프로그램의 PDA와는 다른 주소가 됩니다.

`CommitmentAccount`는 discriminator, commitment, deposited_amount,
withdrawn_amount, ipfs_uri를 저장하며 할당 크기는 260바이트입니다.
IPFS 주소는 온체인 공개 데이터이므로 비밀키나 시드 문구를 포함하지 않습니다.

## Local build

WSL Ubuntu의 프로젝트 루트에서 콜드용 프로그램만 지정합니다.

```bash
cargo check -p cold_wallet --offline
anchor build --program-name cold_wallet
```

루트 `Anchor.toml`의 provider는 기존 mainnet 설정을 유지합니다.
배포할 때는 콜드용 프로그램만 명시적으로 지정해야 합니다.
전체 workspace를 대상으로 하는 무인자 `anchor deploy`는 사용하지 않습니다.

이 프로그램은 온체인 서명 검증과 IPFS 참조 저장을 제공합니다.
개인키의 오프라인 보관 및 오프라인 서명은 클라이언트에서 구현해야 합니다.
