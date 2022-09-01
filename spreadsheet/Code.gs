function onOpen() {
  var ui = SpreadsheetApp.getUi();
  ui.createMenu('Account actions')
    // .addItem('Log a deposit', 'contribute')
    .addItem('Rerun balancing', 'remoteBalance')
    .addItem('Balance without taxed sales', 'noTaxBalance')
    .addItem('Balance without any sales', 'noSalesBalance')
    .addSeparator()
    .addItem('Record current balance', 'populateBalance')
    .addToUi();
}

function contribute() {
  var ui = SpreadsheetApp.getUi();
  var response = ui.prompt('Log a deposit', 'How much did you deposit?', ui.ButtonSet.OK_CANCEL);

  if (response.getSelectedButton() === ui.Button.OK) {
    var ss = SpreadsheetApp.getActive();
    var amount = response.getResponseText();
    try {
      var deposit = parseInt(amount, 10);
      if (!deposit) return;
      var depositRange = ss.getRangeByName("deposit");
      if (depositRange.getValue() > 0) {
        ui.alert("assign the last deposit first");
        return;
      }
      depositRange.setValue(deposit);
      populateBalance(deposit);
    } catch (e) {
      ui.alert("Couldn't handle that amount " + e);
    }
  }
}

function populateBalanceCron() {
  populateBalance(0); // google puts dumb things into arguments, make sure they don't affect us
}

function populateBalance(cashflow) {
  if (!cashflow) cashflow = 0;
  var ss = SpreadsheetApp.getActive();
  var balanceCell = ss.getRangeByName("currentBalance");
  var balance = balanceCell.getCell(1, 1).getValue();
  if (cashflow > 0) balance = balance + cashflow;

  var ledger = ss.getSheetByName("ledger");
  // loop until we find the first row with an empty date
  for (var insertRow = 100; ledger.getRange(insertRow, 1).getValue(); insertRow++) {
  }

  // insert today's date, cashflow & the balance on the empty row
  ledger.getRange(insertRow, 1).setValue(new Date());
  ledger.getRange(insertRow, 2).setValue(cashflow);
  ledger.getRange(insertRow, 4).setValue(balance);

  // insert account balances too
  var accounts = ss.getRangeByName("account_balances");
  var taxed = 8;
  ledger.getRange(insertRow, taxed).setValue(accounts.getCell(1, 1).getValue());
  ledger.getRange(insertRow, taxed + 1).setValue(accounts.getCell(1, 2).getValue());
  ledger.getRange(insertRow, taxed + 2).setValue(accounts.getCell(1, 3).getValue());
  // median div is at col (taxed + 3)
  ledger.getRange(insertRow, taxed + 4).setValue(accounts.getCell(1, 4).getValue());
  ledger.getRange(insertRow, taxed + 5).setValue(accounts.getCell(1, 5).getValue());

  var medianDiv = ss.getRangeByName("median_div");
  ledger.getRange(insertRow, taxed + 3).setValue(medianDiv.getCell(1, 1).getValue());
}

var URL = 'https://etf.gnmerritt.net/balance';

function noTaxBalance() {
  runBalancing(["taxed", "wf"]);
}

function noSalesBalance() {
  runBalancing(["taxed", "roth", "ira", "four01k", "wf"]);
}

function remoteBalance() {
  runBalancing([]);
}

function runBalancing(noSaleAccounts) {
  var accounts = buildAccounts();
  const data = buildData();
  var target = {};
  var market = [];

  for (var i = 0; i < data.stocks.length; i++) {
    var s = data.stocks[i];
    target[s.ticker] = s.percent;
    const investment = {symbol: s.ticker, price: s.price};
    if (s.yield && s.yield > 0) {
      investment.div_yield = s.yield;
    }
    market.push(investment);
  }

  var portfolio = {
    accounts: accounts,
    target: target,
    market: market,
    no_sale_accounts: noSaleAccounts
  };
  var options = {
    'method' : 'post',
    'contentType': 'application/json',
    'payload' : JSON.stringify(portfolio)
  };
  var response = UrlFetchApp.fetch(URL, options);
  var text = response.getContentText();
  var json = JSON.parse(text);

  handleResults(json);
}

function handleResults(results) {
  const ss = SpreadsheetApp.getActive();
  const data = ss.getRangeByName("results");

  for (var i = 1; i <= data.getNumRows(); i++) {
    var symbol = data.getCell(i, SYMBOL).getValue();
    if (!symbol) continue;
    data.getCell(i, TAXED).setValue(balance(results, "taxed", symbol));
    data.getCell(i, ROTH).setValue(balance(results, "roth", symbol));
    data.getCell(i, IRA).setValue(balance(results, "ira", symbol));
    data.getCell(i, FOUR01k).setValue(balance(results, "four01k", symbol));
    data.getCell(i, WF).setValue(balance(results, "wf", symbol));
  }
}

function balance(results, account, symbol) {
  if (symbol.indexOf("Cash") != -1) {
    return results.cash[account];
  }
  var amount = results.positions[account][symbol] || 0.0;
  return amount.toFixed(3);
}

var TAXED = 2;
var ROTH = 3;
var IRA = 4;
var FOUR01k = 5;
var WF = 6;

function buildAccounts() {
  const ss = SpreadsheetApp.getActive();
  const data = ss.getRangeByName("positions");

  var taxed = account("taxed");
  var roth = account("roth", true);
  var ira = account("ira", true);
  var four01k = account("four01k", true);
  var wf = account("wf");

  for (var i = 1; i <= data.getNumRows(); i++) {
    var symbol = data.getCell(i, SYMBOL).getValue();
    if (!symbol) { continue; }
    if (symbol.indexOf("Cash") != -1) {
      taxed.cash = data.getCell(i, TAXED).getValue();
      roth.cash = data.getCell(i, ROTH).getValue();
      ira.cash = data.getCell(i, IRA).getValue();
      four01k.cash = data.getCell(i, FOUR01k).getValue();
      wf.cash = data.getCell(i, WF).getValue();
    } else {
      taxed.positions[symbol] = data.getCell(i, TAXED).getValue();
      roth.positions[symbol] = data.getCell(i, ROTH).getValue();
      ira.positions[symbol] = data.getCell(i, IRA).getValue();
      four01k.positions[symbol] = data.getCell(i, FOUR01k).getValue();
      wf.positions[symbol] = data.getCell(i, WF).getValue();
    }
  }

  return [taxed, roth, ira, four01k, wf];
}

function account(name, tax_sheltered) {
  return {
    name: name,
    tax_sheltered: !!tax_sheltered,
    positions: {},
    cash: 0.0
  };
}

var SYMBOL = 1;
var YIELD = 3;
var PERCENT = 4;
var BALANCED = 5;
var ACTUAL = 6;
var DELTA = 7;
var PRICE = 9;

function buildData() {
  const ss = SpreadsheetApp.getActive();
  const data = ss.getRangeByName("data");

  const stocks = [];
  const symbols = [];

  for (var i = 1; i <= data.getNumRows(); i++) {
    var stock = {
      ticker: data.getCell(i, SYMBOL).getValue(),
      yield: data.getCell(i, YIELD).getValue(),
      percent: data.getCell(i, PERCENT).getValue(),
      price: data.getCell(i, PRICE).getValue(),
      current: data.getCell(i, ACTUAL).getValue(),
      balanced: data.getCell(i, BALANCED).getValue(),
      delta: data.getCell(i, DELTA).getValue(),
      needed: 0
    };
    stocks.push(stock);
    symbols.push(stock.ticker);
  }

  return { stocks: stocks, symbols: symbols };
}
