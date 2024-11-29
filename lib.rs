#![cfg_attr(not(feature = "std"), no_std, no_main)]

#[ink::contract]
mod betting {
    use ink::storage::Mapping;

    // Use BoundedVec?
    pub type TeamName = Vec<u8>;

    const MIN_DEPOSIT: Balance = 1_000_000_000_000;

    #[derive(scale::Decode, scale::Encode, PartialEq, Clone)]
    #[cfg_attr(
        feature = "std",
        derive(scale_info::TypeInfo, ink::storage::traits::StorageLayout)
    )]
    pub enum MatchResult {
        Team1Victory,
        Team2Victory,
        Draw,
    }
    #[derive(scale::Decode, scale::Encode, PartialEq)]
    #[cfg_attr(
        feature = "std",
        derive(scale_info::TypeInfo, ink::storage::traits::StorageLayout)
    )]
    pub struct Bet {
        /// Account of the better.
        pub bettor: AccountId,
        /// Bet amount.
        pub amount: Balance,
        /// Result predicted.
        pub result: MatchResult,
    }
    #[derive(scale::Decode, scale::Encode)]
    #[cfg_attr(
        feature = "std",
        derive(scale_info::TypeInfo, ink::storage::traits::StorageLayout)
    )]
    pub struct Match {
        /// Starting block of the match.
        start: BlockNumber,
        /// Length of the match (start + length = end).
        length: BlockNumber,
        /// Team1 name.
        team1: TeamName,
        /// Team2 name.
        team2: TeamName,
        /// Result.
        result: Option<MatchResult>,
        /// List of bets.
        pub bets: Vec<Bet>,
        /// The amount held in reserve of the `depositor`,
        /// To be returned once this recovery process is closed.
        deposit: Balance,
    }

    #[ink(storage)]
    pub struct Betting {
        /// Mapping of open matches.
        matches: Mapping<AccountId, Match>,
        // Mapping of all match hashes. (hash -> owner)
        //matches_hashes: Mapping<Hash, AccountId>
    }

    /// A new match has been created. [who, team1, team2, start, length]
    #[ink(event)]
    pub struct MatchCreated {
        #[ink(topic)]
        who: AccountId,
        team1: TeamName,
        team2: TeamName,
        start: BlockNumber,
        length: BlockNumber,
    }
    /// A new bet has been created. [matchId, who, amount, result]
    #[ink(event)]
    pub struct BetPlaced {
        #[ink(topic)]
        match_id: AccountId,
        #[ink(topic)]
        who: AccountId,
        amount: Balance,
        result: MatchResult,
    }

    /// The Betting error types.
    #[derive(Debug, PartialEq, Eq, scale::Encode, scale::Decode)]
    #[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
    pub enum Error {
        /// The match to be created already exist.
        MatchAlreadyExists,
        /// Each account can only have one match open.
        OriginHasAlreadyOpenMatch,
        /// The time of the match is over.
        TimeMatchOver,
        /// Not enough deposit to create the Match.
        NotEnoughDeposit,
        /// The match where the bet is placed does not exist
        MatchDoesNotExist,
        /// No allowing betting if the match has started
        MatchHasStarted,
        /// You already place the same bet in that match
        AlreadyBet,
    }

    impl Betting {
        #[ink(constructor)]
        pub fn new() -> Self {
            Self {
                matches: Default::default(),
                //matches_hashes: Default::default(),
            }
        }

        // payable accepts a payment (deposit).
        #[ink(message, payable)]
        pub fn create_match_to_bet(
            &mut self,
            team1: Vec<u8>,
            team2: Vec<u8>,
            start: BlockNumber,
            length: BlockNumber,
        ) -> Result<(), Error> {
            let caller = Self::env().caller();
            // Check account has no open match
            if self.exists_match(caller) {
                return Err(Error::OriginHasAlreadyOpenMatch);
            }
            // Check if start and length are valid
            let current_block_number = self.env().block_number();
            if current_block_number > (start + length) {
                return Err(Error::TimeMatchOver);
            }
            // Check the deposit.
            // Assert or Error?
            let deposit = Self::env().transferred_value();
            if deposit < MIN_DEPOSIT {
                return Err(Error::NotEnoughDeposit);
            }
            // Create the betting match
            let betting_match = Match {
                start,
                length,
                team1,
                team2,
                result: None,
                bets: Default::default(),
                deposit,
            };
            // Check if match already exists by checking its specs hash.
            // How to create a hash of the object betting_match??
            // Store the match hash with its creator account.

            // Store the betting match in the list of open matches
            self.matches.insert(caller, &betting_match);
            // Emit an event.
            self.env().emit_event(MatchCreated {
                who: caller,
                team1: betting_match.team1,
                team2: betting_match.team2,
                start,
                length,
            });

            Ok(())
        }

        // payable accepts a payment (amount_to_bet).
        #[ink(message, payable)]
        pub fn bet(&mut self, match_id: AccountId, result: MatchResult) -> Result<(), Error> {
            let caller = Self::env().caller();
            // Find the match that user wants to place the bet
            let mut match_to_bet = match self.matches.get(&match_id) {
                Some(match_from_storage) => match_from_storage,
                None => return Err(Error::MatchDoesNotExist),
            };

            // Check if the Match Has Started (can't bet in a started match)
            let current_block_number = self.env().block_number();
            if current_block_number > match_to_bet.start {
                return Err(Error::MatchHasStarted);
            }
            let amount = Self::env().transferred_value();
            // Create the bet to be placed
            let bet = Bet {
                bettor: caller,
                amount,
                result: result.clone(),
            };
            // Check if the bet already exists
            if match_to_bet.bets.contains(&bet) {
                return Err(Error::AlreadyBet);
            } else {
                match_to_bet.bets.push(bet);
                // Store the betting match in the list of open matches
                self.matches.insert(match_id, &match_to_bet);
                // Emit an event.
                self.env().emit_event(BetPlaced {
                    match_id,
                    who: caller,
                    amount,
                    result,
                });
            }
            Ok(())
        }

        /// Simply checks if a match exists.
        #[ink(message)]
        pub fn exists_match(&self, owner: AccountId) -> bool {
            self.matches.contains(owner)
        }
        #[ink(message)]
        pub fn get_match(&self, owner: AccountId) -> Option<Match> {
            self.matches.get(owner)
        }
    }
    /// Unit tests in Rust are normally defined within such a `#[cfg(test)]`
    /// module and test functions are marked with a `#[test]` attribute.
    /// The below code is technically just normal Rust code.

    #[cfg(test)]
    mod tests {
        use crate::betting::{Bet, Betting, Error, MatchResult};
        use ink::primitives::AccountId;

        fn create_match(
            betting: &mut Betting,
            who: AccountId,
            t1: &str,
            t2: &str,
            start: u32,
            length: u32,
            deposit: u128,
        ) -> AccountId {
            ink::env::test::set_caller::<ink::env::DefaultEnvironment>(who);
            ink::env::test::set_value_transferred::<ink::env::DefaultEnvironment>(deposit);
            // Dispatch a signed extrinsic.
            assert_eq!(
                betting.create_match_to_bet(
                    t1.as_bytes().to_vec(),
                    t2.as_bytes().to_vec(),
                    start,
                    length
                ),
                Ok(())
            );
            who
        }

        /// We test if the default constructor does its job.
        #[ink::test]
        fn constructor_works() {
            let accounts = ink::env::test::default_accounts::<ink::env::DefaultEnvironment>();
            let betting = Betting::new();
            assert_eq!(betting.exists_match(accounts.alice), false);
        }

        #[ink::test]
        fn create_match_to_bet_works() {
            let accounts = ink::env::test::default_accounts::<ink::env::DefaultEnvironment>();
            let mut betting = Betting::new();

            assert_eq!(betting.exists_match(accounts.alice), false);

            let match_id = create_match(
                &mut betting,
                accounts.alice,
                "team1",
                "team2",
                10,
                10,
                1000000000000,
            );

            assert_eq!(betting.exists_match(match_id), true);

            let emitted_events = ink::env::test::recorded_events().collect::<Vec<_>>();
            assert_eq!(1, emitted_events.len());
        }

        #[ink::test]
        fn not_enough_deposit_when_create_match_to_bet() {
            let accounts = ink::env::test::default_accounts::<ink::env::DefaultEnvironment>();
            let mut betting = Betting::new();

            assert_eq!(betting.exists_match(accounts.alice), false);

            ink::env::test::set_caller::<ink::env::DefaultEnvironment>(accounts.alice);
            ink::env::test::set_value_transferred::<ink::env::DefaultEnvironment>(1);

            assert_eq!(
                betting.create_match_to_bet(
                    "team1".as_bytes().to_vec(),
                    "team2".as_bytes().to_vec(),
                    10,
                    10
                ),
                Err(Error::NotEnoughDeposit)
            );
            assert_eq!(betting.exists_match(accounts.alice), false);
        }

        #[ink::test]
        fn match_exist_when_create_match_to_bet() {
            let accounts = ink::env::test::default_accounts::<ink::env::DefaultEnvironment>();
            let mut betting = Betting::new();

            assert_eq!(betting.exists_match(accounts.alice), false);

            create_match(
                &mut betting,
                accounts.alice,
                "team1",
                "team2",
                10,
                10,
                1000000000000,
            );

            assert_eq!(betting.exists_match(accounts.alice), true);

            //Try to added it again
            assert_eq!(
                betting.create_match_to_bet(
                    "team1".as_bytes().to_vec(),
                    "team2".as_bytes().to_vec(),
                    10,
                    10
                ),
                Err(Error::OriginHasAlreadyOpenMatch)
            );
        }

        #[ink::test]
        fn error_creating_a_match_with_an_open_match() {
            // Advance 3 blocks
            ink::env::test::advance_block::<ink::env::DefaultEnvironment>();
            ink::env::test::advance_block::<ink::env::DefaultEnvironment>();
            ink::env::test::advance_block::<ink::env::DefaultEnvironment>();

            let accounts = ink::env::test::default_accounts::<ink::env::DefaultEnvironment>();
            let mut betting = Betting::new();

            assert_eq!(betting.exists_match(accounts.alice), false);

            ink::env::test::set_caller::<ink::env::DefaultEnvironment>(accounts.alice);
            ink::env::test::set_value_transferred::<ink::env::DefaultEnvironment>(1000000000000);

            assert_eq!(
                betting.create_match_to_bet(
                    "team1".as_bytes().to_vec(),
                    "team2".as_bytes().to_vec(),
                    1,
                    1
                ),
                Err(Error::TimeMatchOver)
            );
            assert_eq!(betting.exists_match(accounts.alice), false);
        }

        #[ink::test]
        fn bet_works() {
            let accounts = ink::env::test::default_accounts::<ink::env::DefaultEnvironment>();
            let mut betting = Betting::new();

            let match_id = create_match(
                &mut betting,
                accounts.alice,
                "team1",
                "team2",
                10,
                10,
                1000000000000,
            );

            ink::env::test::set_caller::<ink::env::DefaultEnvironment>(accounts.bob);
            ink::env::test::set_value_transferred::<ink::env::DefaultEnvironment>(10000000000);
            assert_eq!(betting.bet(match_id, MatchResult::Team1Victory), Ok(()));

            let bet = Bet {
                bettor: accounts.bob,
                amount: 10000000000,
                result: MatchResult::Team1Victory,
            };
            assert_eq!(
                betting.get_match(match_id).unwrap().bets.contains(&bet),
                true
            );

            let emitted_events = ink::env::test::recorded_events().collect::<Vec<_>>();
            assert_eq!(2, emitted_events.len());
        }

        #[ink::test]
        fn bet_error_match_not_exist() {
            let accounts = ink::env::test::default_accounts::<ink::env::DefaultEnvironment>();
            let mut betting = Betting::new();

            ink::env::test::set_caller::<ink::env::DefaultEnvironment>(accounts.bob);
            ink::env::test::set_value_transferred::<ink::env::DefaultEnvironment>(10000000000);
            assert_eq!(
                betting.bet(accounts.alice, MatchResult::Team1Victory),
                Err(Error::MatchDoesNotExist)
            );
        }

        #[ink::test]
        fn bet_error_match_has_starte() {
            let accounts = ink::env::test::default_accounts::<ink::env::DefaultEnvironment>();
            let mut betting = Betting::new();

            create_match(
                &mut betting,
                accounts.alice,
                "team1",
                "team2",
                1,
                10,
                1000000000000,
            );
            // Advance 2 blocks
            ink::env::test::advance_block::<ink::env::DefaultEnvironment>();
            ink::env::test::advance_block::<ink::env::DefaultEnvironment>();

            ink::env::test::set_caller::<ink::env::DefaultEnvironment>(accounts.bob);
            ink::env::test::set_value_transferred::<ink::env::DefaultEnvironment>(10000000000);
            assert_eq!(
                betting.bet(accounts.alice, MatchResult::Team1Victory),
                Err(Error::MatchHasStarted)
            );
        }

        #[ink::test]
        fn bet_error_duplicate_bet() {
            let accounts = ink::env::test::default_accounts::<ink::env::DefaultEnvironment>();
            let mut betting = Betting::new();

            let match_id = create_match(
                &mut betting,
                accounts.alice,
                "team1",
                "team2",
                10,
                10,
                1000000000000,
            );

            ink::env::test::set_caller::<ink::env::DefaultEnvironment>(accounts.bob);
            ink::env::test::set_value_transferred::<ink::env::DefaultEnvironment>(10000000000);
            assert_eq!(betting.bet(match_id, MatchResult::Team1Victory), Ok(()));

            assert_eq!(
                betting.bet(match_id, MatchResult::Team1Victory),
                Err(Error::AlreadyBet)
            );
        }
    }
    /// This is how you'd write end-to-end (E2E) or integration tests for ink! contracts.
    ///
    /// When running these you need to make sure that you:
    /// - Compile the tests with the `e2e-tests` feature flag enabled (`--features e2e-tests`)
    /// - Are running a Substrate node which contains `pallet-contracts` in the background
    #[cfg(all(test, feature = "e2e-tests"))]
    mod e2e_tests {
        /// Imports all the definitions from the outer scope so we can use them here.
        use super::*;

        /// A helper function used for calling contract messages.
        use ink_e2e::ContractsBackend;

        /// The End-to-End test `Result` type.
        type E2EResult<T> = std::result::Result<T, Box<dyn std::error::Error>>;

        /// We test that we can upload and instantiate the contract using its default constructor.
        #[ink_e2e::test]
        async fn default_works(mut client: ink_e2e::Client<C, E>) -> E2EResult<()> {
            // Given
            let mut constructor = BettingRef::default();

            // When
            let contract = client
                .instantiate("betting", &ink_e2e::alice(), &mut constructor)
                .submit()
                .await
                .expect("instantiate failed");
            let call_builder = contract.call_builder::<Betting>();

            // Then
            let get = call_builder.get();
            let get_result = client.call(&ink_e2e::alice(), &get).dry_run().await?;
            assert!(matches!(get_result.return_value(), false));

            Ok(())
        }

        /// We test that we can read and write a value from the on-chain contract.
        #[ink_e2e::test]
        async fn it_works(mut client: ink_e2e::Client<C, E>) -> E2EResult<()> {
            // Given
            let mut constructor = BettingRef::new(false);
            let contract = client
                .instantiate("betting", &ink_e2e::bob(), &mut constructor)
                .submit()
                .await
                .expect("instantiate failed");
            let mut call_builder = contract.call_builder::<Betting>();

            let get = call_builder.get();
            let get_result = client.call(&ink_e2e::bob(), &get).dry_run().await?;
            assert!(matches!(get_result.return_value(), false));

            // When
            let flip = call_builder.flip();
            let _flip_result = client
                .call(&ink_e2e::bob(), &flip)
                .submit()
                .await
                .expect("flip failed");

            // Then
            let get = call_builder.get();
            let get_result = client.call(&ink_e2e::bob(), &get).dry_run().await?;
            assert!(matches!(get_result.return_value(), true));

            Ok(())
        }
    }
}
