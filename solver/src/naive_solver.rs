mod multi_order_solver;
mod single_pair_settlement;

use self::single_pair_settlement::SinglePairSettlement;
use crate::settlement::Settlement;
use anyhow::{anyhow, Result};
use contracts::{GPv2Settlement, UniswapV2Factory, UniswapV2Pair, UniswapV2Router02};
use ethcontract::Address;
use model::{order::OrderCreation, TokenPair};
use primitive_types::U256;
use std::collections::HashMap;

pub async fn settle(
    orders: impl Iterator<Item = OrderCreation>,
    uniswap_router: &UniswapV2Router02,
    uniswap_factory: &UniswapV2Factory,
    gpv2_settlement: &GPv2Settlement,
) -> Option<Settlement> {
    let orders = organize_orders_by_token_pair(orders);
    // TODO: Settle multiple token pairs in one settlement.
    for (pair, orders) in orders {
        if let Some(settlement) = settle_pair(pair, orders, &uniswap_factory).await {
            return Some(
                settlement.into_settlement(uniswap_router.clone(), gpv2_settlement.clone()),
            );
        }
    }
    None
}

async fn settle_pair(
    pair: TokenPair,
    orders: Vec<OrderCreation>,
    factory: &UniswapV2Factory,
) -> Option<SinglePairSettlement> {
    let reserves = match get_reserves(pair.get().0, pair.get().1, factory).await {
        Ok(reserves) => reserves,
        Err(err) => {
            tracing::warn!("Error getting AMM reserves: {}", err);
            return None;
        }
    };
    Some(multi_order_solver::solve(orders.into_iter(), &reserves))
}

async fn get_reserves(
    token_0: Address,
    token_1: Address,
    uniswap_factory: &UniswapV2Factory,
) -> Result<HashMap<Address, U256>> {
    let pair = uniswap_factory.get_pair(token_0, token_1).call().await?;
    if pair == Address::zero() {
        return Err(anyhow!("No pool found"));
    }
    let pair = UniswapV2Pair::at(&uniswap_factory.raw_instance().web3(), pair);
    let reserves = pair.get_reserves().call().await?;

    // Token Pairs have a canonical ordering in which reserves are returned (https://github.com/Uniswap/uniswap-v2-core/blob/4dd59067c76dea4a0e8e4bfdda41877a6b16dedc/contracts/UniswapV2Factory.sol#L25)
    let (token_0, token_1) = if token_0 < token_1 {
        (token_0, token_1)
    } else {
        (token_1, token_0)
    };
    Ok(maplit::hashmap! {
        token_0 => U256::from(reserves.0),
        token_1 => U256::from(reserves.1),
    })
}

fn organize_orders_by_token_pair(
    orders: impl Iterator<Item = OrderCreation>,
) -> HashMap<TokenPair, Vec<OrderCreation>> {
    let mut result = HashMap::<_, Vec<OrderCreation>>::new();
    for (order, token_pair) in orders
        .filter(usable_order)
        .filter_map(|order| Some((order, order.token_pair()?)))
    {
        result.entry(token_pair).or_default().push(order);
    }
    result
}

fn usable_order(order: &OrderCreation) -> bool {
    !order.sell_amount.is_zero() && !order.buy_amount.is_zero()
}
