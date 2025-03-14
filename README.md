## Payment Splitter Smart Contract in Ink! for Polkadot

This smart contract enables automatic fund distribution to multiple recipients. It is written in Ink! without third-party libraries. Key features:

- **Multi-payee splitting**: Distribute funds evenly (with remainder handling)  
- **Designated authority**: Single account controls payout triggers  
- **Reentrancy protection**: Prevents recursive attacks  
- **Transparent accounting**: On-chain event logging

### Code Walkthrough

#### Error Handling

```rust
#[derive(Debug, PartialEq, Eq, scale::Encode, scale::Decode)]
pub enum Error {
        Unauthorized = 0, /// Indicates that the caller is not authorized to perform the requested action.
        NoPayees = 1, /// Indicates that there are no payees registered in the contract.
        TransferFailed = 2, /// Indicates that the transfer of funds to a payee failed.
        ZeroShare = 3, /// Indicates that a zero value was provided where a non-zero value was expected.
        ReentrancyGuardLocked = 4,/// Reentrancy guard is locked.
}
```

#### Core Structures

```rust
#[ink(storage)]
pub struct PaymentSplitter {
    payees: Vec<AccountId>,        // Like list of output addresses
    designated_payee: AccountId,    // Similar to script signer
    locked: bool                     // Reentrancy guard 
}

#[derive(Debug, PartialEq, Eq, scale::Encode, scale::Decode)]
pub struct PayoutInfo {             
    pub payee: AccountId,           // Recipient address
    pub amount: Balance,            
}
```

#### Constructor

```rust
#[ink(constructor)]
pub fn new(payees: Vec<AccountId>, designated_payee: AccountId) -> Self {
    Self {
        payees,                     // Set once (like script params)
        designated_payee,           // Immutable authority
        locked: false,               // Initial unlocked state
    }
}
```

Key features:

1. **Storage Structure**:
- `payees`: List of predefined beneficiary addresses
- `designated_payee`: Only address allowed to trigger payouts

2. **Core Functions**:
- `calculate_payout`: Calculates the payout distribution among the registered payees
- `trigger_payout`: Distributes contract balance equally to payees

3. **Security**:
- Only designated payee can trigger distributions
- Proper error handling for edge cases
- Safe balance calculations with overflow protection