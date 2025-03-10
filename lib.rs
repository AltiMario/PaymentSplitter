#![cfg_attr(not(feature = "std"), no_std, no_main)]

#[ink::contract]
mod payment_splitter {
    use ink::prelude::vec::Vec;

    /// Represents the possible errors that can occur within the PaymentSplitter contract.
    #[derive(Debug, PartialEq, Eq, scale::Encode, scale::Decode)]
    #[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
    pub enum Error {
        /// Indicates that the caller is not authorized to perform the requested action.
        Unauthorized = 0,
        /// Indicates that there are no payees registered in the contract.
        NoPayees = 1,
        /// Indicates that the transfer of funds to a payee failed.
        TransferFailed = 2,
        /// Indicates that a zero value was provided where a non-zero value was expected.
        ZeroShare = 3,
        /// Reentrancy guard is locked.
        ReentrancyGuardLocked = 4,
    }

    /// Struct to hold the amount to be transferred for each payee.
    #[derive(Debug, PartialEq, Eq, scale::Encode, scale::Decode)]
    #[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
    pub struct PayoutInfo {
        pub payee: AccountId,
        pub amount: Balance,
    }

    /// Defines the storage for the PaymentSplitter contract.
    #[ink(storage)]
    pub struct PaymentSplitter {
        /// A list of `AccountId`s representing the payees who will receive funds.
        payees: Vec<AccountId>,
        /// The `AccountId` that is authorized to trigger the payout process.
        designated_payee: AccountId,
        /// Reentrancy guard.
        locked: bool,
    }

    /// An event emitted when funds are deposited into the contract.
    #[ink::event]
    pub struct Deposit {
        /// The `AccountId` that deposited the funds.
        #[ink(topic)]
        pub from: AccountId,
        /// The amount of funds deposited.
        pub value: Balance,
    }

    impl PaymentSplitter {
        /// Constructor to initialize the PaymentSplitter contract.
        ///
        /// This constructor sets up the contract with a list of payees and an authorized payee.
        ///
        /// # Arguments
        ///
        /// * `payees`: A vector of `AccountId`s representing the payees who will receive payments.
        /// * `designated_payee`: The `AccountId` that is authorized to trigger the payout.
        ///
        #[ink(constructor)]
        pub fn new(payees: Vec<AccountId>, designated_payee: AccountId) -> Self {
            Self {
                payees,
                designated_payee,
                locked: false,
            }
        }

        /// Allows anyone to deposit funds into the contract.
        ///
        /// The deposited amount is added to the contract's balance.
        /// Emits a `Deposit` event when funds are received, recording the depositor and the amount.
        ///
        /// # Errors
        ///
        /// * `ZeroShare`: If the transferred value is zero.
        ///
        #[ink(message, payable)]
        pub fn deposit(&self) -> Result<(), Error> {
            let transferred_value = self.env().transferred_value();
            if transferred_value == 0 {
                return Err(Error::ZeroShare);
            }
            self.env().emit_event(Deposit {
                from: self.env().caller(),
                value: transferred_value,
            });
            Ok(())
        }

        /// Calculates the payout distribution among the registered payees.
        ///
        /// This function determines how much each payee should receive based on the contract's balance.
        /// The remainder after division is added to the first payee's share.
        ///
        /// # Errors
        ///
        /// * `Unauthorized`: If the caller is not the `designated_payee`.
        /// * `NoPayees`: If there are no registered payees.
        /// * `ZeroShare`: If the total balance is zero or if a calculation error (division by zero) occurs.
        ///
    //    #[ink(message)]
        pub fn calculate_payout(&mut self) -> Result<Vec<PayoutInfo>, Error> {
            self.ensure_caller_is_designated_payee()?;
            let total_balance = self.env().balance();
            let num_payees = self.payees.len();

            if num_payees == 0 {
                return Err(Error::NoPayees);
            }

            if total_balance == 0 {
                return Err(Error::ZeroShare);
            }

            // Calculate the share each payee should receive.
            let share = total_balance.checked_div(num_payees as u128).ok_or(Error::ZeroShare)?;

            if share == 0 {
                return Err(Error::ZeroShare);
            }

            // Calculate the remainder after division.
            let mut remainder = total_balance.saturating_sub(
                share.checked_mul(num_payees as u128).ok_or(Error::ZeroShare)?
            );

            let mut payout_info = Vec::new();
            for (i, payee) in self.payees.iter().enumerate() {
                // Add the remainder to the first payee's share.
                let to_transfer = if i == 0 {
                    share.checked_add(remainder).ok_or(Error::TransferFailed)?
                } else {
                    share
                };

                payout_info.push(PayoutInfo {
                    payee: *payee,
                    amount: to_transfer,
                });

                // Only add remainder to first payee.
                remainder = 0;
            }
            Ok(payout_info)
        }

        /// Triggers the actual payout process based on the payout distribution calculated by `calculate_payout`.
        ///
        /// Only the `designated_payee` is authorized to call this function.
        /// Transfers the funds to each payee based on the `PayoutInfo` provided.
        ///
        /// # Errors
        ///
        /// * `Unauthorized`: If the caller is not the `designated_payee`.
        /// * `TransferFailed`: If the transfer of funds to a payee fails.
        ///
        #[ink(message)]
        pub fn trigger_payout(&mut self) -> Result<(), Error> {
            self.ensure_caller_is_designated_payee()?;
            self.ensure_reentrancy_guard_not_locked()?;

            self.locked = true;
            let payout_info = self.calculate_payout()?;

            for info in payout_info {
                self
                    .env()
                    .transfer(info.payee, info.amount)
                    .map_err(|_| Error::TransferFailed)?;
            }
            self.locked = false;
            Ok(())
        }

        /// Helper function to check if the caller is the designated payee.
        fn ensure_caller_is_designated_payee(&self) -> Result<(), Error> {
            if self.env().caller() != self.designated_payee {
                return Err(Error::Unauthorized);
            }
            Ok(())
        }

        /// Helper function to check the reentrancy guard.
        fn ensure_reentrancy_guard_not_locked(&self) -> Result<(), Error> {
            if self.locked {
                return Err(Error::ReentrancyGuardLocked);
            }
            Ok(())
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use ink::env::test;
        use ink::codegen::Env;

        // Helper function to get the current balance of an account.
        fn get_balance(account: AccountId) -> u128 {
            test::get_account_balance::<ink::env::DefaultEnvironment>(account).expect(
                "Cannot get account balance"
            )
        }

        // Helper function to get the default accounts for testing.
        fn default_accounts() -> test::DefaultAccounts<ink::env::DefaultEnvironment> {
            test::default_accounts::<ink::env::DefaultEnvironment>()
        }

        #[ink::test]
        fn trigger_payout_unauthorized() {
            // Arrange
            let accounts = default_accounts();
            let payees = vec![accounts.bob, accounts.charlie];
            let mut contract = PaymentSplitter::new(payees.clone(), accounts.alice);
            test::set_caller::<ink::env::DefaultEnvironment>(accounts.charlie);
            test::set_value_transferred::<ink::env::DefaultEnvironment>(100);
            contract.deposit().unwrap();

            // Act - Payout
            test::set_caller::<ink::env::DefaultEnvironment>(accounts.bob); // Bob is not the designated_payee
            let result = contract.calculate_payout();

            // Assert
            assert_eq!(result, Err(Error::Unauthorized));
        }

        #[ink::test]
        fn basic_workflow() {
            // Arrange
            let accounts = default_accounts();
            let payees = vec![accounts.bob, accounts.charlie];
            let mut contract = PaymentSplitter::new(payees.clone(), accounts.alice);

            // Set initial values
            let initial_contract_balance = 1000000;
            let bob_balance = 2000010;
            let charlie_balance = 3000010;
            let alice_deposit = 121;
            let balance_plus_deposit = initial_contract_balance + alice_deposit;
            let expected_bob_received = 500061;
            let expected_charlie_received = 500060;

            // Set initial balances
            test::set_account_balance::<ink::env::DefaultEnvironment>(
                contract.env().account_id(),
                initial_contract_balance
            );
            test::set_account_balance::<ink::env::DefaultEnvironment>(accounts.bob, bob_balance);
            test::set_account_balance::<ink::env::DefaultEnvironment>(
                accounts.charlie,
                charlie_balance
            );

            // Act - Deposit
            test::set_caller::<ink::env::DefaultEnvironment>(accounts.alice);
            test::set_value_transferred::<ink::env::DefaultEnvironment>(alice_deposit);
            contract.deposit().unwrap();

            // Update contract balance after Alice deposit
            test::set_account_balance::<ink::env::DefaultEnvironment>(
                contract.env().account_id(),
                balance_plus_deposit
            );

            //Get balances before payout
            let contract_balance_before_split = get_balance(contract.env().account_id());
            ink::env::debug_println!("---- Contract balance before split: {}",contract_balance_before_split);

            // Assert - Deposit (still Alice as caller)
            assert_eq!(get_balance(contract.env().account_id()), balance_plus_deposit);

            // Calculate Payout
            let payout_info = contract.calculate_payout().unwrap();
            ink::env::debug_println!("---- payout info 0: {:?}", payout_info[0].amount);
            ink::env::debug_println!("---- payout info 1: {:?}", payout_info[1].amount);

            // Assert Payout Calculations
            assert_eq!(payout_info.len(), 2);
            assert_eq!(payout_info[0].payee, accounts.bob);
            assert_eq!(payout_info[0].amount, expected_bob_received);
            assert_eq!(payout_info[1].payee, accounts.charlie);
            assert_eq!(payout_info[1].amount, expected_charlie_received);

            // Trigger Payout
            contract.trigger_payout().unwrap();

            //Get balances after payout
            let contract_balance_after = get_balance(contract.env().account_id());
            ink::env::debug_println!("---- Contract balance after split: {}",contract_balance_after);

            //Update balances after payout (should be 0 after payout)
            test::set_account_balance::<ink::env::DefaultEnvironment>(
                contract.env().account_id(),
                0
            );

            // Assert - Payout
            assert_eq!(get_balance(contract.env().account_id()), 0);
            assert_eq!(get_balance(accounts.bob), bob_balance + expected_bob_received);
            assert_eq!(get_balance(accounts.charlie), charlie_balance + expected_charlie_received);
            ink::env::debug_println!("---- Bob balance: {}", get_balance(accounts.bob));
            ink::env::debug_println!("---- Charlie balance: {}", get_balance(accounts.charlie));
        }
    }
}
