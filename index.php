<!DOCTYPE html>
<html xmlns="http://www.w3.org/1999/xhtml" xml:lang="fr" lang="fr">
<head>
<title>Bons comptes</title>
<meta charset="utf-8"/>
<style>
body
{
  background-image: url("images/billets.jpg");
  background-position: center top;
}

p
{
  background-color:#eeeeff;
  opacity:1;
  filter:alpha(opacity=100); /* For IE8 and earlier */
}

table
{
  border-collapse:collapse;
  background-color:#ffffff;
  opacity:0.75;
  filter:alpha(opacity=75); /* For IE8 and earlier */
}

th,td
{
  border: 1px solid black;
}

h1
{
  background-color:#fefeff;
  opacity:1;
  filter:alpha(opacity=100); /* For IE8 and earlier */
}

h2
{
  background-color:#eeeeff;
  opacity:0.75;
  filter:alpha(opacity=75); /* For IE8 and earlier */
}
</style>
</head>
<body>
<?php
  include_once('./functions.php');
  $nodebt = db_connect();

  echo("<h1>Bons comptes</h1>\n");
  $user = get_string('user');

  echo("<h2>Actions:</h2>\n");
  $raw_user = get_raw_string('user');
  echo("<p><a href='./insert.php?user=$raw_user'>\n");
  echo("Je viens d'effectuer un paiement, d'autres devraient contribuer\n</a><br />\n");
  echo("<a href='./insert.php?user=$raw_user&contrib_num=0'>\n");
  echo("Je viens de rembourser quelqu'un\n</a><br />\n");
  echo("<a href='./insert.php?receiver=$raw_user'>\n");
  echo("Je viens de recevoir de l'argent, d'autres devraient être remboursés\n</a></p>\n");

  echo("<p><a href='./details.php?user=$raw_user'>\n");
  echo("Plus de détails\n</a></p>\n");

  print_table("Vous doit de l'argent&nbsp;:", $nodebt, "
      SELECT fname, uname, ROUND(debt,2) AS debt
      FROM money_debt
      WHERE viewer_uname = '$user'
      AND user IS NOT NULL
      AND debt >= 0.01
      ORDER BY debt DESC, fname
    ",
    array("Personne", "Dette&nbsp;($)"),
    array("fname", "debt"),
    array("fname" => "uname")
  );

  print_table("Vous lui devez de l'argent&nbsp;:", $nodebt, "
      SELECT fname, uname, ROUND(-debt,2) AS debt
      FROM money_debt
      WHERE viewer_uname = '$user'
      AND user IS NOT NULL
      AND debt <= -0.01
      ORDER BY debt DESC, fname
    ",
    array("Personne", "Dette&nbsp;($)"),
    array("fname", "debt"),
    array("fname" => "uname")
  );

  print_table("Transactions en attente de remboursement", $nodebt, "
      SELECT payer_fname, payer_uname, DATE(date) AS date, description,
        ROUND(money,2) AS money,
        (
          CASE
          WHEN (debt - 0.01 < add_sum) THEN 100
          WHEN (add_sum < debt - money + 0.01) THEN 0
          ELSE ROUND((money-debt+add_sum)/money*100,2)
          END
        ) AS percent_left,
        (
          CASE
          WHEN (debt - 0.01 < add_sum) THEN 0
          WHEN (add_sum < debt - money + 0.01) THEN ROUND(money,2)
          ELSE ROUND(debt-add_sum,2)
          END
        ) AS money_left
      FROM money_addsum
      WHERE
      (
        date > NOW() - INTERVAL 1 MONTH
        OR debt - 0.01 >= add_sum
      )
      AND receiver_uname = '$user'
      ORDER BY date DESC, money DESC
    ",
    array("Personne", "Date", "Description", "Montant&nbsp;($)",
          "Reste à payer&nbsp;($)", "%"),
    array("payer_fname", "date", "description", "money",
          "money_left", "percent_left"),
    array("payer_fname" => "payer_uname")
  );

  print_table("Transactions à rembourser", $nodebt, "
      SELECT receiver_fname, receiver_uname, DATE(date) AS date, description,
        ROUND(money,2) AS money,
        (
          CASE
          WHEN (debt - 0.01 < add_sum) THEN 100
          WHEN (add_sum < debt - money + 0.01) THEN 0
          ELSE ROUND((money-debt+add_sum)/money*100,2)
          END
        ) AS percent_left,
        (
          CASE
          WHEN (debt - 0.01 < add_sum) THEN 0
          WHEN (add_sum < debt - money + 0.01) THEN ROUND(money,2)
          ELSE ROUND(debt-add_sum,2)
          END
        ) AS money_left
      FROM money_addsum
      WHERE
      (
        date > NOW() - INTERVAL 1 MONTH
        OR debt - 0.01 >= add_sum
      )
      AND payer_uname = '$user'

      ORDER BY date DESC, money DESC
    ",
    array("Personne", "Date", "Description", "Montant&nbsp;($)",
          "Reste à payer&nbsp;($)", "%"),
    array("receiver_fname", "date", "description", "money",
          "money_left", "percent_left"),
    array("receiver_fname" => "payer_uname")
  );

  $nodebt->close();

?>
</body>
</html>
