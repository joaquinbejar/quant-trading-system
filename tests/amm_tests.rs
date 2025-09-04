use quant_trading_system::domain::{amm_pool::*, types::*};
use quant_trading_system::AMMPoolUpdate;
use rust_decimal_macros::dec;

#[test]
fn test_amm_pool_v2_creation() {
    let address = PoolAddress("0x123".to_string());
    let reserves = TokenReserves::new(dec!(1000.0), dec!(2000.0));
    let fee_tier = FeeTier(30);

    let pool = ThreadSafeAMMPool::new_v2(address.clone(), reserves, fee_tier);

    assert_eq!(pool.get_address(), address);
    assert_eq!(pool.get_reserves().token0, dec!(1000.0));
    assert_eq!(pool.get_reserves().token1, dec!(2000.0));
    assert_eq!(pool.get_fee_tier(), fee_tier);
    assert_eq!(pool.pool_type(), AMMPoolType::ConstantProduct);
}

#[test]
fn test_amm_pool_v3_creation() {
    let address = PoolAddress("0x456".to_string());
    let reserves = TokenReserves::new(dec!(500.0), dec!(1500.0));
    let sqrt_price = SqrtPriceX96(2u128.pow(96) * 2); // sqrt(4) * 2^96
    let fee_tier = FeeTier(5);

    let pool = ThreadSafeAMMPool::new_v3(address.clone(), reserves, sqrt_price, fee_tier);

    assert_eq!(pool.get_address(), address);
    assert_eq!(pool.pool_type(), AMMPoolType::ConcentratedLiquidity);
}

#[test]
fn test_implied_mid_price_v2() {
    let address = PoolAddress("0x789".to_string());
    let reserves = TokenReserves::new(dec!(1000.0), dec!(2000.0));
    let fee_tier = FeeTier(30);

    let pool = ThreadSafeAMMPool::new_v2(address, reserves, fee_tier);
    let mid_price = pool.implied_mid();

    // Price should be token1/token0 = 2000/1000 = 2.0
    assert_eq!(mid_price, Price(dec!(2.0)));
}

#[test]
fn test_implied_mid_price_v3_with_sqrt_price() {
    let address = PoolAddress("0xABC".to_string());
    let reserves = TokenReserves::new(dec!(1000.0), dec!(2000.0));
    let sqrt_price = SqrtPriceX96(2u128.pow(96)); // sqrt(1) * 2^96 = price of 1
    let fee_tier = FeeTier(5);

    let pool = ThreadSafeAMMPool::new_v3(address, reserves, sqrt_price, fee_tier);
    let mid_price = pool.implied_mid();

    // Should use sqrt_price, not reserves
    assert!((mid_price.0 - dec!(1.0)).abs() < dec!(0.1));
}

#[test]
fn test_price_impact_calculation() {
    let address = PoolAddress("0xDEF".to_string());
    let reserves = TokenReserves::new(dec!(1000.0), dec!(2000.0));
    let fee_tier = FeeTier(30);

    let pool = ThreadSafeAMMPool::new_v2(address, reserves, fee_tier);

    // Test small trade impact
    let result = pool.calculate_price_impact(TokenIndex::Token0, dec!(10.0));
    assert!(result.is_ok());

    let impact = result.unwrap();
    assert!(impact.output_amount > dec!(0.0));
    assert!(impact.price_impact_percent >= dec!(0.0));
    assert!(impact.fee_amount > dec!(0.0));
}

#[test]
fn test_amm_pool_state_update() {
    let address = PoolAddress("0x111".to_string());
    let initial_reserves = TokenReserves::new(dec!(1000.0), dec!(2000.0));
    let fee_tier = FeeTier(30);

    let pool = ThreadSafeAMMPool::new_v2(address, initial_reserves, fee_tier);

    // Update reserves
    let new_reserves = TokenReserves::new(dec!(1100.0), dec!(1900.0));
    let update = AMMPoolUpdate::new(chrono::Utc::now(), new_reserves, None, fee_tier);

    let result = pool.update_state(update);
    assert!(result.is_ok());

    // Verify updated state
    assert_eq!(pool.get_reserves().token0, dec!(1100.0));
    assert_eq!(pool.get_reserves().token1, dec!(1900.0));
}

#[test]
fn test_zero_reserves_handling() {
    let address = PoolAddress("0x222".to_string());
    let reserves = TokenReserves::new(dec!(0.0), dec!(1000.0));
    let fee_tier = FeeTier(30);

    let pool = ThreadSafeAMMPool::new_v2(address, reserves, fee_tier);
    let mid_price = pool.implied_mid();

    // Should return zero price when token0 reserve is zero
    assert_eq!(mid_price, Price::zero());
}

#[test]
fn test_large_trade_price_impact() {
    let address = PoolAddress("0x333".to_string());
    let reserves = TokenReserves::new(dec!(1000.0), dec!(2000.0));
    let fee_tier = FeeTier(30);

    let pool = ThreadSafeAMMPool::new_v2(address, reserves, fee_tier);

    // Test large trade (50% of pool)
    let result = pool.calculate_price_impact(TokenIndex::Token0, dec!(500.0));
    assert!(result.is_ok());

    let impact = result.unwrap();
    // Large trades should have significant price impact
    assert!(impact.price_impact_percent > dec!(10.0));
}

#[test]
fn test_fee_tier_conversion() {
    let fee_30 = FeeTier(30); // 30 basis points = 0.3%
    assert_eq!(fee_30.to_decimal(), dec!(0.003));

    let fee_5 = FeeTier(5); // 5 basis points = 0.05%
    assert_eq!(fee_5.to_decimal(), dec!(0.0005));
}

#[test]
fn test_token_reserves_operations() {
    let reserves1 = TokenReserves::new(dec!(1000.0), dec!(2000.0));
    let reserves2 = TokenReserves::new(dec!(500.0), dec!(1000.0));

    assert_eq!(reserves1.token0, dec!(1000.0));
    assert_eq!(reserves1.token1, dec!(2000.0));
    assert_ne!(reserves1, reserves2);
}
