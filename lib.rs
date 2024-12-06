#![cfg_attr(not(feature = "std"), no_std, no_main)]

#[ink::contract]
mod betting {
    use ink::storage::Mapping;

    // Use BoundedVec?
    pub type TeamName = Vec<u8>;

    const MIN_DEPOSIT: Balance = 1_000_000_000_000;

    #[derive(scale::Decode, scale::Encode, PartialEq, Clone, Copy)]
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
        /// Owner of the Smart Contract (sudo)
        owner: AccountId,
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
    /// A match result has been set. [matchId, result]
    #[ink(event)]
    pub struct MatchResultSet {
        #[ink(topic)]
        match_id: AccountId,
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
        /// Only owner of the smart contract can make this call
        BadOrigin,
        /// No allowing set the result if the match not over
        TimeMatchNotOver,
        /// The match still has not a result set
        MatchNotResult,
        /// Returned if the requested transfer failed. This can be the case if the
        /// contract does not have sufficient free funds or if the transfer would
        /// have brought the contract's balance below minimum balance.
        TransferFailed,
    }

    impl Betting {
        #[ink(constructor, payable)]
        pub fn new() -> Self {
            let owner = Self::env().caller();
            Self {
                matches: Default::default(),
                owner, //matches_hashes: Default::default(),
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
            let mut match_to_bet = match self.matches.take(&match_id) {
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

        /// Set the result of an existing match.
        /// The dispatch origin for this call must be the owner.
        /// Get root of the node?? like ensure_root(origin)?;
        #[ink(message)]
        pub fn set_result(
            &mut self,
            match_id: AccountId,
            result: MatchResult,
        ) -> Result<(), Error> {
            let caller = Self::env().caller();
            // Only owner of the SC can call this message.
            if caller != self.owner {
                return Err(Error::BadOrigin);
            }
            //Find the match where owner wants to set the result
            let mut match_to_set_result = match self.matches.take(&match_id) {
                Some(match_from_storage) => match_from_storage,
                None => return Err(Error::MatchDoesNotExist),
            };
            // Check if start and length are valid
            let current_block_number = self.env().block_number();
            if current_block_number <= (match_to_set_result.start + match_to_set_result.length) {
                return Err(Error::TimeMatchNotOver);
            }
            //set the result
            match_to_set_result.result = Some(result.clone());
            // Store the betting match in the list of open matches
            self.matches.insert(match_id, &match_to_set_result);
            // Emit an event.
            self.env().emit_event(MatchResultSet { match_id, result });

            Ok(())
        }

        /// When a match ends the owner of the match can distribute funds to the winners and delete the match.
        #[ink(message)]
        pub fn distribute_winnings(&mut self) -> Result<(), Error> {
            let caller = Self::env().caller();
            // Get the match that user wants to close, deleting it
            let mut match_to_delete = match self.matches.take(&caller) {
                Some(match_from_storage) => match_from_storage,
                None => return Err(Error::MatchDoesNotExist),
            };
            // Make sure the match has a result set already
            if !match_to_delete.result.is_some() {
                return Err(Error::MatchNotResult);
            }
            // Iterate over all bets to get the winners accounts
            let mut total_winners: Balance = 0u32.into();
            let mut total_bet: Balance = 0u32.into();
            let mut winners = Vec::new();
            for bet in match_to_delete.bets.iter_mut() {
                total_bet += bet.amount;
                if Some(bet.result) == match_to_delete.result {
                    total_winners += bet.amount;
                    winners.push(bet)
                }
            }
            // Distribute funds
            for winner_bet in &winners {
                let weighted = winner_bet.amount / (total_winners / 100);
                let amount_won = weighted * (total_bet / 100);
                self.env()
                    .transfer(winner_bet.bettor, amount_won)
                    .map_err(|_| Error::TransferFailed)?;
            }
            // Return deposit
            self.env()
                .transfer(caller, match_to_delete.deposit)
                .map_err(|_| Error::TransferFailed)?;

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

        fn set_accounts() -> ink::env::test::DefaultAccounts<ink::env::DefaultEnvironment> {
            let accounts = ink::env::test::default_accounts::<ink::env::DefaultEnvironment>();
            ink::env::test::set_account_balance::<ink::env::DefaultEnvironment>(
                accounts.alice,
                100000000000000,
            );
            ink::env::test::set_account_balance::<ink::env::DefaultEnvironment>(
                accounts.bob,
                100000000000000,
            );
            ink::env::test::set_account_balance::<ink::env::DefaultEnvironment>(
                accounts.charlie,
                100000000000000,
            );
            ink::env::test::set_account_balance::<ink::env::DefaultEnvironment>(
                accounts.django,
                100000000000000,
            );
            ink::env::test::set_account_balance::<ink::env::DefaultEnvironment>(
                accounts.eve,
                100000000000000,
            );
            accounts
        }

        fn create_contract(who: AccountId) -> Betting {
            ink::env::test::set_caller::<ink::env::DefaultEnvironment>(who);
            ink::env::test::transfer_in::<ink::env::DefaultEnvironment>(1000000000000);
            let betting = Betting::new();
            betting
        }

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
            ink::env::test::transfer_in::<ink::env::DefaultEnvironment>(deposit);
            // ink::env::test::transfer_in::<ink::env::DefaultEnvironment>(deposit);
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
            let accounts = set_accounts();
            let betting = create_contract(accounts.alice);
            assert_eq!(betting.exists_match(accounts.alice), false);
        }

        #[ink::test]
        fn create_match_to_bet_works() {
            let accounts = set_accounts();
            let mut betting = create_contract(accounts.alice);

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
            let accounts = set_accounts();
            let mut betting = create_contract(accounts.alice);

            assert_eq!(betting.exists_match(accounts.alice), false);

            ink::env::test::set_caller::<ink::env::DefaultEnvironment>(accounts.alice);
            ink::env::test::transfer_in::<ink::env::DefaultEnvironment>(1);

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
            let accounts = set_accounts();
            let mut betting = create_contract(accounts.alice);

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

            let accounts = set_accounts();
            let mut betting = create_contract(accounts.alice);

            assert_eq!(betting.exists_match(accounts.alice), false);

            ink::env::test::set_caller::<ink::env::DefaultEnvironment>(accounts.alice);
            ink::env::test::transfer_in::<ink::env::DefaultEnvironment>(1000000000000);

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
            let accounts = set_accounts();
            let mut betting = create_contract(accounts.alice);

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
            ink::env::test::transfer_in::<ink::env::DefaultEnvironment>(10000000000);
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
            let accounts = set_accounts();
            let mut betting = create_contract(accounts.alice);

            ink::env::test::set_caller::<ink::env::DefaultEnvironment>(accounts.bob);
            ink::env::test::transfer_in::<ink::env::DefaultEnvironment>(10000000000);
            assert_eq!(
                betting.bet(accounts.alice, MatchResult::Team1Victory),
                Err(Error::MatchDoesNotExist)
            );
        }

        #[ink::test]
        fn bet_error_match_has_start() {
            let accounts = set_accounts();
            let mut betting = create_contract(accounts.alice);

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
            ink::env::test::transfer_in::<ink::env::DefaultEnvironment>(10000000000);
            assert_eq!(
                betting.bet(accounts.alice, MatchResult::Team1Victory),
                Err(Error::MatchHasStarted)
            );
        }

        #[ink::test]
        fn bet_error_duplicate_bet() {
            let accounts = set_accounts();
            let mut betting = create_contract(accounts.alice);

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
            ink::env::test::transfer_in::<ink::env::DefaultEnvironment>(10000000000);
            assert_eq!(betting.bet(match_id, MatchResult::Team1Victory), Ok(()));

            assert_eq!(
                betting.bet(match_id, MatchResult::Team1Victory),
                Err(Error::AlreadyBet)
            );
        }

        #[ink::test]
        fn set_result_works() {
            let accounts = set_accounts();
            let mut betting = create_contract(accounts.alice);

            let match_id = create_match(
                &mut betting,
                accounts.alice,
                "team1",
                "team2",
                1,
                1,
                1000000000000,
            );

            assert_eq!(betting.exists_match(match_id), true);

            // Advance 3 blocks
            ink::env::test::advance_block::<ink::env::DefaultEnvironment>();
            ink::env::test::advance_block::<ink::env::DefaultEnvironment>();
            ink::env::test::advance_block::<ink::env::DefaultEnvironment>();
            assert_eq!(
                betting.set_result(match_id, MatchResult::Team1Victory),
                Ok(())
            );

            let emitted_events = ink::env::test::recorded_events().collect::<Vec<_>>();
            assert_eq!(2, emitted_events.len());
        }
        #[ink::test]
        fn set_result_bad_origin() {
            let accounts = set_accounts();
            let mut betting = create_contract(accounts.alice);

            let match_id = create_match(
                &mut betting,
                accounts.alice,
                "team1",
                "team2",
                1,
                1,
                1000000000000,
            );

            // Advance 3 blocks
            ink::env::test::advance_block::<ink::env::DefaultEnvironment>();
            ink::env::test::advance_block::<ink::env::DefaultEnvironment>();
            ink::env::test::advance_block::<ink::env::DefaultEnvironment>();
            //set Bob as the caller
            ink::env::test::set_caller::<ink::env::DefaultEnvironment>(accounts.bob);
            assert_eq!(
                betting.set_result(match_id, MatchResult::Team1Victory),
                Err(Error::BadOrigin)
            );
        }
        #[ink::test]
        fn set_result_match_not_exist() {
            let accounts = set_accounts();
            let mut betting = create_contract(accounts.alice);

            // Advance 3 blocks
            ink::env::test::advance_block::<ink::env::DefaultEnvironment>();
            ink::env::test::advance_block::<ink::env::DefaultEnvironment>();
            ink::env::test::advance_block::<ink::env::DefaultEnvironment>();
            assert_eq!(
                betting.set_result(accounts.alice, MatchResult::Team1Victory),
                Err(Error::MatchDoesNotExist)
            );
        }
        #[ink::test]
        fn set_result_match_not_finished() {
            let accounts = set_accounts();
            let mut betting = create_contract(accounts.alice);
            let match_id = create_match(
                &mut betting,
                accounts.alice,
                "team1",
                "team2",
                10,
                10,
                1000000000000,
            );

            assert_eq!(
                betting.set_result(match_id, MatchResult::Team1Victory),
                Err(Error::TimeMatchNotOver)
            );
        }

        #[ink::test]
        fn distribute_winnings_works() {
            let accounts = set_accounts();
            let mut betting = create_contract(accounts.alice);

            //Django creates the match
            let match_id = create_match(
                &mut betting,
                accounts.django,
                "team1",
                "team2",
                1,
                1,
                1000000000000,
            );

            assert_eq!(betting.exists_match(match_id), true);
            // Bob bets
            ink::env::test::set_caller::<ink::env::DefaultEnvironment>(accounts.bob);
            ink::env::test::transfer_in::<ink::env::DefaultEnvironment>(10000000000000);
            assert_eq!(betting.bet(match_id, MatchResult::Team1Victory), Ok(()));
            // Charlie bets
            ink::env::test::set_caller::<ink::env::DefaultEnvironment>(accounts.charlie);
            ink::env::test::transfer_in::<ink::env::DefaultEnvironment>(10000000000000);
            assert_eq!(betting.bet(match_id, MatchResult::Team2Victory), Ok(()));
            // Eve bets
            ink::env::test::set_caller::<ink::env::DefaultEnvironment>(accounts.eve);
            ink::env::test::transfer_in::<ink::env::DefaultEnvironment>(30000000000000);
            assert_eq!(betting.bet(match_id, MatchResult::Team1Victory), Ok(()));

            // Advance 3 blocks
            ink::env::test::advance_block::<ink::env::DefaultEnvironment>();
            ink::env::test::advance_block::<ink::env::DefaultEnvironment>();
            ink::env::test::advance_block::<ink::env::DefaultEnvironment>();
            //Alice set the result
            ink::env::test::set_caller::<ink::env::DefaultEnvironment>(accounts.alice);
            assert_eq!(
                betting.set_result(match_id, MatchResult::Team1Victory),
                Ok(())
            );
            //Django distributes the winnings
            ink::env::test::set_caller::<ink::env::DefaultEnvironment>(accounts.django);
            assert_eq!(betting.distribute_winnings(), Ok(()));
            //bob has 90 + 12.5 (winner)
            assert_eq!(
                ink::env::test::get_account_balance::<ink::env::DefaultEnvironment>(accounts.bob),
                Ok(102500000000000)
            );
            //charlie has 90 (loser)
            assert_eq!(
                ink::env::test::get_account_balance::<ink::env::DefaultEnvironment>(
                    accounts.charlie
                ),
                Ok(90000000000000)
            );
            //eve has 90 + 37.5 (winner)
            assert_eq!(
                ink::env::test::get_account_balance::<ink::env::DefaultEnvironment>(accounts.eve),
                Ok(107500000000000)
            );
        }

        #[ink::test]
        fn distribute_winnings_match_not_exist() {
            let accounts = set_accounts();
            let mut betting = create_contract(accounts.alice);

            //Django creates the match
            let match_id = create_match(
                &mut betting,
                accounts.django,
                "team1",
                "team2",
                1,
                1,
                1000000000000,
            );

            assert_eq!(betting.exists_match(match_id), true);
            // Bob bets
            ink::env::test::set_caller::<ink::env::DefaultEnvironment>(accounts.bob);
            ink::env::test::transfer_in::<ink::env::DefaultEnvironment>(10000000000000);
            assert_eq!(betting.bet(match_id, MatchResult::Team1Victory), Ok(()));
            // Charlie bets
            ink::env::test::set_caller::<ink::env::DefaultEnvironment>(accounts.charlie);
            ink::env::test::transfer_in::<ink::env::DefaultEnvironment>(10000000000000);
            assert_eq!(betting.bet(match_id, MatchResult::Team2Victory), Ok(()));
            // Eve bets
            ink::env::test::set_caller::<ink::env::DefaultEnvironment>(accounts.eve);
            ink::env::test::transfer_in::<ink::env::DefaultEnvironment>(30000000000000);
            assert_eq!(betting.bet(match_id, MatchResult::Team1Victory), Ok(()));

            // Advance 3 blocks
            ink::env::test::advance_block::<ink::env::DefaultEnvironment>();
            ink::env::test::advance_block::<ink::env::DefaultEnvironment>();
            ink::env::test::advance_block::<ink::env::DefaultEnvironment>();
            //Alice set the result
            ink::env::test::set_caller::<ink::env::DefaultEnvironment>(accounts.alice);
            assert_eq!(
                betting.set_result(match_id, MatchResult::Team1Victory),
                Ok(())
            );

            //alice distribute winner doesn't exists
            assert_eq!(betting.distribute_winnings(), Err(Error::MatchDoesNotExist));
        }

        #[ink::test]
        fn distribute_winnings_match_not_result_yet() {
            let accounts = set_accounts();
            let mut betting = create_contract(accounts.alice);

            //Django creates the match
            let match_id = create_match(
                &mut betting,
                accounts.django,
                "team1",
                "team2",
                1,
                1,
                1000000000000,
            );

            assert_eq!(betting.exists_match(match_id), true);
            // Bob bets
            ink::env::test::set_caller::<ink::env::DefaultEnvironment>(accounts.bob);
            ink::env::test::transfer_in::<ink::env::DefaultEnvironment>(10000000000000);
            assert_eq!(betting.bet(match_id, MatchResult::Team1Victory), Ok(()));
            // Charlie bets
            ink::env::test::set_caller::<ink::env::DefaultEnvironment>(accounts.charlie);
            ink::env::test::transfer_in::<ink::env::DefaultEnvironment>(10000000000000);
            assert_eq!(betting.bet(match_id, MatchResult::Team2Victory), Ok(()));
            // Eve bets
            ink::env::test::set_caller::<ink::env::DefaultEnvironment>(accounts.eve);
            ink::env::test::transfer_in::<ink::env::DefaultEnvironment>(30000000000000);
            assert_eq!(betting.bet(match_id, MatchResult::Team1Victory), Ok(()));

            //Django distributes the winnings
            ink::env::test::set_caller::<ink::env::DefaultEnvironment>(accounts.django);
            assert_eq!(betting.distribute_winnings(), Err(Error::MatchNotResult));
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
